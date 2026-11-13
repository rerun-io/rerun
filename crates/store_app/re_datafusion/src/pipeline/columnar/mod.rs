//! Columnar emission: output columns built directly from the chunks' Arrow
//! arrays instead of resolving rows one at a time through `QueryHandle`.
//!
//! Per emit window it collects, for every column of the view, which chunk
//! row supplies each timestamp (max `RowId` wins, a `Clear` counts as an
//! empty value, a static value shadows everything, a static `Clear` nulls the
//! column at every time), takes the union of those timestamps as the row
//! grid, and materializes each output column with one gather over the chunk
//! arrays; no output row is ever assembled on its own.
//!
//! The rules are those of `pipeline/reference.rs`, which also lists where
//! they differ from `QueryHandle`.
//!
//! The chunks stay in a [`SegmentStore`], whose insert, byte accounting and
//! GC are reused; only emission differs. Queries this path does not cover
//! yet (`fill_latest_at`, `using_index_values`, `filtered_index_values` and
//! static-only queries) are rejected by [`columnar_index`], and
//! [`super::emitter::SegmentRowEmitter`] runs them on the store's
//! `QueryHandle` path instead.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, Int64Array, ListArray, MutableArrayData, RecordBatch, RecordBatchOptions,
    StringArray, make_array, new_empty_array, new_null_array,
};
use arrow::buffer::OffsetBuffer;
use arrow::compute::{cast, interleave};
use arrow::datatypes::{DataType, Field, Schema};
use re_dataframe::external::re_chunk::{Chunk, ChunkId, RangeQuery, RowId, TimeColumn};
use re_dataframe::external::re_chunk_store::{ChunkStore, ChunkTrackingMode};
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{QueryExpression, SparseFillStrategy};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimelineName};
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_redap_client::{ApiError, ApiResult};
use re_sorbet::{BatchType, ComponentColumnDescriptor};
use re_types_core::archetypes;
use tokio::sync::mpsc::Sender;

use crate::dataframe_query_provider::segment_store::{
    FLUSH_BATCH_BYTES, FLUSH_BATCH_ROWS, SegmentStore,
};

/// The timeline to build on, or `None` when the query needs the
/// `QueryHandle` path.
pub fn columnar_index(query: &QueryExpression) -> Option<TimelineName> {
    let index = query.filtered_index?;
    let needs_query_handle = query.sparse_fill_strategy != SparseFillStrategy::None
        || query.using_index_values.is_some()
        || query.filtered_index_values.is_some();
    (!needs_query_handle).then_some(index)
}

/// Emit every row of `store` on `index` in `(processed_through, horizon]`
/// (`None` = to the end), then mark the range processed.
pub async fn emit(
    store: &mut SegmentStore,
    index: TimelineName,
    horizon: Option<TimeInt>,
    projected_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit_rows: Option<usize>,
) -> ApiResult<()> {
    let Some(range) = store.next_emit_range(horizon) else {
        return Ok(());
    };
    let origin = store.origin.clone();

    let window = {
        let chunks = store.store.read();
        Window::collect(
            &chunks,
            &store.query_expression,
            index,
            range,
            projected_schema,
        )
    };
    store.mark_processed(range);

    let segment_id = store.segment_id.as_str();
    let row_bytes = window.estimated_row_bytes(projected_schema);
    let mut start = 0;
    while start < window.grid.len() {
        let remaining = limit_rows.map_or(usize::MAX, |l| l.saturating_sub(*rows_sent));
        if remaining == 0 {
            return Ok(());
        }
        let end = next_batch_end(
            &row_bytes,
            start,
            FLUSH_BATCH_ROWS.min(remaining),
            FLUSH_BATCH_BYTES,
        );
        let batch = window
            .batch(start..end, segment_id, projected_schema)
            .map_err(|err| {
                ApiError::internal_with_source(
                    &origin,
                    None,
                    err,
                    "building a columnar output batch",
                )
            })?;
        let batch = align_record_batch_to_schema(&batch, projected_schema).map_err(|err| {
            ApiError::internal_with_source(&origin, None, err, "DataFusion schema mismatch error")
        })?;
        *rows_sent += batch.num_rows();
        output_channel.send(batch).await.map_err(|err| {
            ApiError::internal_with_source(&origin, None, err, "output channel closed")
        })?;
        start = end;
    }
    Ok(())
}

/// The end of the output batch that starts at grid row `start`: at most
/// `max_rows` rows, cut after the row that brings the estimate to `max_bytes`.
/// Always at least one row, so a single row over `max_bytes` still ships.
fn next_batch_end(row_bytes: &[usize], start: usize, max_rows: usize, max_bytes: usize) -> usize {
    let mut bytes = 0;
    let mut end = start;
    for &row in row_bytes.iter().skip(start).take(max_rows) {
        end += 1;
        bytes += row;
        if bytes >= max_bytes {
            break;
        }
    }
    end
}

/// The rows of `times` that can lie inside `range`: a binary-searched span
/// when the column is sorted, every row otherwise. Callers still check each
/// row's time against `range`.
fn candidate_rows(times: &TimeColumn, range: AbsoluteTimeRange) -> std::ops::Range<usize> {
    let raw = times.times_raw();
    if times.is_sorted() {
        let start = raw.partition_point(|&t| t < range.min().as_i64());
        let end = raw.partition_point(|&t| t <= range.max().as_i64());
        start..end
    } else {
        0..raw.len()
    }
}

/// Which source row supplies one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    /// A `Clear` won: the row still carries that chunk's other timelines.
    Empty {
        chunk: usize,
        row: usize,
    },

    Value {
        chunk: usize,
        row: usize,
    },
}

impl Cell {
    fn source(self) -> (usize, usize) {
        match self {
            Self::Empty { chunk, row } | Self::Value { chunk, row } => (chunk, row),
        }
    }
}

/// One temporal `Clear` row that applies to an entity, with the chunk it
/// lives in so its other timelines stay reachable.
struct ClearRow {
    time: TimeInt,
    row_id: RowId,
    chunk: Arc<Chunk>,
    row: usize,
}

/// The `Clear`s logged on one entity.
#[derive(Default)]
struct EntityClears {
    temporal: Vec<ClearRow>,

    /// The max-`RowId` static `Clear`, which hides every temporal value and
    /// every static value logged before it.
    static_row_id: Option<RowId>,
}

/// One view column's contribution to a window.
struct ColumnRows {
    descr: ComponentColumnDescriptor,

    /// `descr`'s dataframe column name, what projected fields are matched on.
    name: String,

    /// Every chunk a cell can point into, data and clear chunks alike, each
    /// once.
    chunks: Vec<Arc<Chunk>>,

    /// The max-`RowId` static row, which shadows every temporal one.
    static_cell: Option<Cell>,

    /// A static `Clear` hides the column at every time, and no static value
    /// was logged after it. The temporal rows still create grid rows but add
    /// nothing to the other-timeline columns (reference rule 6).
    cleared: bool,

    /// The winning cell per timestamp in the window, ascending, one per
    /// timestamp. Kept sparse: a column's cells are only resolved against
    /// the grid one output batch at a time.
    per_time: Vec<(TimeInt, Cell)>,
}

impl ColumnRows {
    /// The temporal cells that land on `times`, an ascending slice of the
    /// grid, as `(position in times, cell)`. Static and cleared columns are
    /// left to the caller.
    fn temporal_cells_on<'a>(
        &'a self,
        times: &'a [TimeInt],
    ) -> impl Iterator<Item = (usize, Cell)> + 'a {
        let (first, last) = match times {
            [] => (TimeInt::MAX, TimeInt::MIN),
            [first, .., last] => (*first, *last),
            [only] => (*only, *only),
        };
        let start = self.per_time.partition_point(|(t, _)| *t < first);
        let end = self.per_time.partition_point(|(t, _)| *t <= last);
        let mut i = 0;
        self.per_time[start..end.max(start)]
            .iter()
            .filter_map(move |&(t, cell)| {
                i += times[i..].partition_point(|&g| g < t);
                (times.get(i) == Some(&t)).then_some((i, cell))
            })
    }

    /// The column's cell on each of `times`, an ascending slice of the grid;
    /// `None` is null.
    fn cells_on(&self, times: &[TimeInt]) -> Vec<Option<Cell>> {
        if self.cleared {
            return vec![None; times.len()];
        }
        if let Some(cell) = self.static_cell {
            return vec![Some(cell); times.len()];
        }
        let mut cells = vec![None; times.len()];
        for (i, cell) in self.temporal_cells_on(times) {
            cells[i] = Some(cell);
        }
        cells
    }

    /// The column's cells on `times`, an ascending slice of the grid,
    /// gathered from the chunk arrays they point into.
    ///
    /// The output has the view's datatype for the column, as `QueryHandle`'s
    /// does. Rows from a chunk logged with another datatype are an error
    /// unless they are the column's only rows in `times`.
    fn materialize(&self, times: &[TimeInt]) -> Result<ArrayRef, arrow::error::ArrowError> {
        let datatype = &self.descr.store_datatype;
        let mut sources: Vec<ArrayRef> = Vec::new();
        let mut source_of_chunk: Vec<Option<usize>> = vec![None; self.chunks.len()];
        let mut empty_source = None;

        let indices: Vec<Option<(usize, usize)>> = self
            .cells_on(times)
            .into_iter()
            .map(|cell| match cell {
                Some(Cell::Value { chunk, row }) => {
                    let source = *source_of_chunk[chunk].get_or_insert_with(|| {
                        let list = self.chunks[chunk]
                            .components()
                            .get_array(self.descr.component)
                            .expect("a Value cell points at a chunk with the component");
                        sources.push(Arc::new(list.clone()));
                        sources.len() - 1
                    });
                    Some((source, row))
                }
                Some(Cell::Empty { .. }) => {
                    let source = *empty_source.get_or_insert_with(|| {
                        sources.push(empty_list(datatype));
                        sources.len() - 1
                    });
                    Some((source, 0))
                }
                None => None,
            })
            .collect();
        gather(datatype, &sources, &indices)
    }
}

/// A one-row empty list of `datatype`, what a `Clear` shows; a null for a
/// datatype that is not a list.
///
/// The list field is the view's own, so the sentinel has exactly `datatype`
/// and can be gathered together with the chunks' arrays.
fn empty_list(datatype: &DataType) -> ArrayRef {
    match datatype {
        DataType::List(field) => Arc::new(ListArray::new(
            field.clone(),
            OffsetBuffer::new_zeroed(1),
            new_empty_array(field.data_type()),
            None,
        )),
        _ => new_null_array(datatype, 1),
    }
}

/// Everything needed to materialize the rows of one emit window.
struct Window {
    index: TimelineName,

    /// The projected other timelines, by output column name, with one value per
    /// grid row: the max over the rows that row's cells show (reference rule
    /// 6), so a column showing a static cell or hidden by a static `Clear`
    /// adds nothing.
    other_indices: Vec<(String, Vec<Option<i64>>)>,

    /// The output timestamps, ascending.
    grid: Vec<TimeInt>,

    /// The view's columns, which decide the grid and the other-timeline
    /// values; a subset of them is projected.
    columns: Vec<ColumnRows>,

    /// Index into `columns` by dataframe column name.
    column_by_name: ahash::HashMap<String, usize>,
}

impl Window {
    fn collect(
        store: &ChunkStore,
        query: &QueryExpression,
        index: TimelineName,
        range: AbsoluteTimeRange,
        projected_schema: &Schema,
    ) -> Self {
        re_tracing::profile_function!();

        let range_query = RangeQuery::new(index, range);
        let clear_component = archetypes::Clear::descriptor_is_recursive().component;

        let view = store.schema_for_query(query);

        // A clear on the entity empties every column of that entity at its
        // time; a static one at every time. A clear on an ancestor never
        // applies, whatever its recursive flag says.
        let clears_per_entity: BTreeMap<&EntityPath, EntityClears> = view
            .components
            .iter()
            .map(|descr| &descr.entity_path)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|entity_path| {
                let mut clears = EntityClears::default();
                for chunk in store
                    .range_relevant_chunks(
                        ChunkTrackingMode::Ignore,
                        &range_query,
                        entity_path,
                        clear_component,
                    )
                    .chunks
                {
                    let Some(flags) = chunk.components().get_array(clear_component) else {
                        continue;
                    };
                    let times = chunk.timelines().get(&index);
                    if times.is_none() && !chunk.is_static() {
                        continue;
                    }
                    let rows =
                        times.map_or_else(|| 0..chunk.num_rows(), |t| candidate_rows(t, range));
                    for row in rows.filter(|&row| !flags.is_null(row)) {
                        let row_id = chunk.row_ids_slice()[row];
                        match times {
                            Some(times) => {
                                let t = TimeInt::new_temporal(times.times_raw()[row]);
                                if range.contains(t) {
                                    clears.temporal.push(ClearRow {
                                        time: t,
                                        row_id,
                                        chunk: Arc::clone(&chunk),
                                        row,
                                    });
                                }
                            }
                            None => clears.static_row_id = clears.static_row_id.max(Some(row_id)),
                        }
                    }
                }
                (entity_path, clears)
            })
            .collect();
        let columns = view
            .components
            .iter()
            .map(|descr| {
                // A `Clear` never clears its own column (reference rule 3).
                let clears = clears_per_entity
                    .get(&descr.entity_path)
                    .filter(|_| descr.component != clear_component);
                let static_clear = clears.and_then(|c| c.static_row_id);
                let mut chunks: Vec<Arc<Chunk>> = Vec::new();
                let mut chunk_idx_of: ahash::HashMap<ChunkId, usize> = Default::default();
                let mut chunk_idx = |chunk: &Arc<Chunk>| {
                    *chunk_idx_of.entry(chunk.id()).or_insert_with(|| {
                        chunks.push(Arc::clone(chunk));
                        chunks.len() - 1
                    })
                };
                let mut static_cell: Option<(RowId, Cell)> = None;
                let mut facts: Vec<(TimeInt, RowId, Cell)> = Vec::new();

                for chunk in store
                    .range_relevant_chunks(
                        ChunkTrackingMode::Ignore,
                        &range_query,
                        &descr.entity_path,
                        descr.component,
                    )
                    .chunks
                {
                    let Some(list) = chunk.components().get_array(descr.component) else {
                        continue;
                    };
                    let chunk_idx = chunk_idx(&chunk);
                    if chunk.is_static() {
                        for row in 0..chunk.num_rows() {
                            if list.is_null(row) {
                                continue;
                            }
                            let row_id = chunk.row_ids_slice()[row];
                            if static_clear.is_none_or(|c| row_id > c)
                                && static_cell.is_none_or(|(best, _)| row_id > best)
                            {
                                static_cell = Some((
                                    row_id,
                                    Cell::Value {
                                        chunk: chunk_idx,
                                        row,
                                    },
                                ));
                            }
                        }
                    } else if let Some(times) = chunk.timelines().get(&index) {
                        let raw = times.times_raw();
                        for row in candidate_rows(times, range) {
                            let t = TimeInt::new_temporal(raw[row]);
                            if range.contains(t) && !list.is_null(row) {
                                facts.push((
                                    t,
                                    chunk.row_ids_slice()[row],
                                    Cell::Value {
                                        chunk: chunk_idx,
                                        row,
                                    },
                                ));
                            }
                        }
                    }
                }

                for clear in clears.into_iter().flat_map(|c| &c.temporal) {
                    facts.push((
                        clear.time,
                        clear.row_id,
                        Cell::Empty {
                            chunk: chunk_idx(&clear.chunk),
                            row: clear.row,
                        },
                    ));
                }

                // Ascending `(time, RowId)`, so the last fact per time is the
                // max-`RowId` one. A value and a `Clear` logged in one row tie
                // on both; the `Clear` sorts last and wins.
                facts.sort_unstable_by_key(|(t, row_id, cell)| {
                    (*t, *row_id, matches!(cell, Cell::Empty { .. }))
                });
                let mut per_time: Vec<(TimeInt, Cell)> = Vec::with_capacity(facts.len());
                for (t, _, cell) in facts {
                    match per_time.last_mut() {
                        Some((last, winner)) if *last == t => *winner = cell,
                        _ => per_time.push((t, cell)),
                    }
                }

                ColumnRows {
                    name: descr.column_name(BatchType::Dataframe),
                    descr: descr.clone(),
                    chunks,
                    cleared: static_clear.is_some() && static_cell.is_none(),
                    static_cell: static_cell.map(|(_, cell)| cell),
                    per_time,
                }
            })
            .collect::<Vec<_>>();

        let grid_source: Vec<&ColumnRows> = match &query.filtered_is_not_null {
            Some(not_null) => columns
                .iter()
                .filter(|c| c.descr.matches(not_null))
                .collect(),
            None => columns.iter().collect(),
        };
        let mut grid: Vec<TimeInt> = grid_source
            .iter()
            .flat_map(|c| c.per_time.iter().map(|(t, _)| *t))
            .collect();
        grid.sort_unstable();
        grid.dedup();

        let other_indices = view
            .indices
            .iter()
            .map(|descr| *descr.timeline().name())
            .filter(|timeline| {
                *timeline != index && projected_schema.field_with_name(timeline.as_str()).is_ok()
            })
            .map(|timeline| {
                let mut values = vec![None; grid.len()];
                // A static cell, or a static `Clear`, hides the temporal rows.
                for column in columns
                    .iter()
                    .filter(|c| !c.cleared && c.static_cell.is_none())
                {
                    let chunk_times: Vec<Option<&[i64]>> = column
                        .chunks
                        .iter()
                        .map(|c| c.timelines().get(&timeline).map(|t| t.times_raw()))
                        .collect();
                    for (i, cell) in column.temporal_cells_on(&grid) {
                        let (chunk, row) = cell.source();
                        if let Some(times) = chunk_times[chunk] {
                            values[i] = values[i].max(Some(times[row]));
                        }
                    }
                }
                (timeline.to_string(), values)
            })
            .collect();

        let column_by_name = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i))
            .collect();

        Self {
            index,
            other_indices,
            grid,
            columns,
            column_by_name,
        }
    }

    /// The output batch for `rows` of the grid, with the projected schema's
    /// column names and the arrays' own datatypes.
    ///
    /// Projected fields are matched to view columns by name, the same rule
    /// `align_record_batch_to_schema` applies afterwards; a projected column
    /// the view does not have is all null.
    fn batch(
        &self,
        rows: std::ops::Range<usize>,
        segment_id: &str,
        projected_schema: &Schema,
    ) -> Result<RecordBatch, arrow::error::ArrowError> {
        re_tracing::profile_function!();

        let times = &self.grid[rows.clone()];
        let mut fields = Vec::with_capacity(projected_schema.fields().len());
        let mut arrays = Vec::with_capacity(projected_schema.fields().len());

        for field in projected_schema.fields() {
            let name = field.name().as_str();
            let array: ArrayRef = if name == ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME
            {
                Arc::new(StringArray::from(vec![segment_id; times.len()]))
            } else if name == self.index.as_str() {
                let raw = Int64Array::from_iter_values(times.iter().map(|t| t.as_i64()));
                cast(&raw, field.data_type())?
            } else if let Some((_, values)) = self.other_indices.iter().find(|(n, _)| n == name) {
                let raw = Int64Array::from(values[rows.clone()].to_vec());
                cast(&raw, field.data_type())?
            } else if let Some(column) = self.column(name) {
                column.materialize(times)?
            } else {
                new_null_array(field.data_type(), times.len())
            };
            fields.push(Field::new(field.name(), array.data_type().clone(), true));
            arrays.push(array);
        }

        RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(fields, Default::default())),
            arrays,
            &RecordBatchOptions::default().with_row_count(Some(times.len())),
        )
    }

    fn column(&self, name: &str) -> Option<&ColumnRows> {
        self.column_by_name.get(name).map(|&i| &self.columns[i])
    }

    /// Estimated output bytes of each grid row over the projected component
    /// columns, counting each temporal cell as its source chunk's average
    /// bytes per row and a static cell, copied onto every row, at its own
    /// size. Index and segment-id columns are small and left out.
    fn estimated_row_bytes(&self, projected_schema: &Schema) -> Vec<usize> {
        re_tracing::profile_function!();

        let mut row_bytes = vec![0; self.grid.len()];
        for field in projected_schema.fields() {
            let Some(column) = self.column(field.name()) else {
                continue;
            };
            let per_chunk: Vec<usize> = column
                .chunks
                .iter()
                .map(|c| {
                    c.components()
                        .get_array(column.descr.component)
                        .map_or(0, |a| a.get_array_memory_size() / a.len().max(1))
                })
                .collect();
            let static_bytes = column.static_cell.map(|cell| {
                let (chunk, row) = cell.source();
                column.chunks[chunk]
                    .components()
                    .get_array(column.descr.component)
                    .map_or(0, |list| {
                        // The row's own element: a one-row slice of the list
                        // would count the list's whole child buffer, every
                        // other static row's values included. Types whose
                        // slice size arrow cannot compute count as the whole
                        // element, an overestimate.
                        let element = list.value(row);
                        element
                            .to_data()
                            .get_slice_memory_size()
                            .unwrap_or_else(|_err| element.get_array_memory_size())
                    })
            });
            if column.cleared {
                continue;
            }
            if let Some(static_bytes) = static_bytes {
                for bytes in &mut row_bytes {
                    *bytes += static_bytes;
                }
                continue;
            }
            for (i, cell) in column.temporal_cells_on(&self.grid) {
                if let Cell::Value { chunk, .. } = cell {
                    row_bytes[i] += per_chunk[chunk];
                }
            }
        }
        row_bytes
    }
}

/// The array of `datatype` whose element `i` is row `indices[i]` of
/// `sources`, `(source, row)`, or null for `None`.
///
/// Consecutive rows of one source, and consecutive nulls, are copied as one
/// run. A single run is a zero-copy slice, the common case of a range query
/// over one chunk; otherwise runs are copied whole, which for list and binary
/// columns is one copy per run rather than one per child element. Only when
/// no two indices merge does it fall back to `interleave`.
///
/// A single run is returned with its source's own datatype; otherwise every
/// source must have exactly `datatype`, or the gather is an error.
fn gather(
    datatype: &DataType,
    sources: &[ArrayRef],
    indices: &[Option<(usize, usize)>],
) -> Result<ArrayRef, arrow::error::ArrowError> {
    re_tracing::profile_function!();

    // `None` runs are nulls, with only the range's length meaningful.
    let mut runs: Vec<(Option<usize>, std::ops::Range<usize>)> = Vec::new();
    for &index in indices {
        let (source, row) = match index {
            Some((source, row)) => (Some(source), row),
            None => (None, 0),
        };
        match runs.last_mut() {
            Some((last, rows)) if *last == source && (source.is_none() || rows.end == row) => {
                rows.end += 1;
            }
            _ => runs.push((source, row..row + 1)),
        }
    }

    match runs.as_slice() {
        [] => Ok(arrow::array::new_empty_array(datatype)),
        [(None, rows)] => Ok(new_null_array(datatype, rows.len())),
        [(Some(source), rows)] => Ok(sources[*source].slice(rows.start, rows.len())),
        _ if let Some(other) = sources.iter().find(|s| s.data_type() != datatype) => {
            Err(arrow::error::ArrowError::InvalidArgumentError(format!(
                "cannot gather a column of {datatype} from a chunk of {}",
                other.data_type()
            )))
        }
        _ if runs.len() == indices.len() => {
            let null_source = new_null_array(datatype, 1);
            let null_idx = sources.len();
            let refs: Vec<&dyn Array> =
                std::iter::chain(sources.iter().map(|a| a.as_ref()), [null_source.as_ref()])
                    .collect();
            let indices: Vec<(usize, usize)> =
                indices.iter().map(|i| i.unwrap_or((null_idx, 0))).collect();
            interleave(&refs, &indices)
        }
        _ => {
            let data: Vec<arrow::array::ArrayData> = sources.iter().map(|a| a.to_data()).collect();
            let mut mutable = MutableArrayData::new(data.iter().collect(), true, indices.len());
            for (source, rows) in &runs {
                match source {
                    Some(source) => mutable.extend(*source, rows.start, rows.end),
                    None => mutable.extend_nulls(rows.len()),
                }
            }
            Ok(make_array(mutable.freeze()))
        }
    }
}

#[cfg(test)]
mod tests;
