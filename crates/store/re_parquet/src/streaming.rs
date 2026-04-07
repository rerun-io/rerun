//! Core batch→chunk conversion with an iterator-based streaming API.

use std::collections::VecDeque;
use std::sync::Arc;

use arrow::array::{
    Array, FixedSizeListArray, Float32Array, Float64Array, RecordBatch, RecordBatchReader as _,
};
use arrow::datatypes::{DataType, Field};
use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint};
// Component: for KeyValuePairs::name(), ComponentBatch: for .try_serialized()
use re_sdk_types::{Component as _, ComponentBatch as _, ComponentDescriptor, datatypes};

use crate::config::{MappedComponent, ParquetConfig};
use crate::grouping::{ColumnGroup, ColumnGroupEntry, compute_column_groups};
use crate::timeline::{self, TimelineInfo};

const PARQUET_METADATA_ARCHETYPE: &str = "ParquetMetadata";

/// Errors that can occur during parquet loading.
#[derive(Debug, thiserror::Error)]
pub enum ParquetError {
    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Load a parquet file from disk and return a chunk iterator.
pub(crate) fn load_from_path(
    path: &std::path::Path,
    config: &ParquetConfig,
    entity_path_prefix: &EntityPath,
) -> Result<ParquetChunkIterator, ParquetError> {
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let file =
        std::fs::File::open(path).map_err(|err| ParquetError::from(anyhow::Error::from(err)))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|err| ParquetError::from(anyhow::Error::from(err)))?;

    let metadata = builder.metadata().clone();
    let reader = builder
        .build()
        .map_err(|err| ParquetError::from(anyhow::Error::from(err)))?;
    let schema = reader.schema().clone();

    build_iterator(
        Box::new(reader),
        schema,
        &metadata,
        config,
        entity_path_prefix.clone(),
    )
}

/// Load parquet from in-memory bytes and return a chunk iterator.
pub(crate) fn load_from_bytes(
    bytes: &[u8],
    config: &ParquetConfig,
    entity_path_prefix: &EntityPath,
) -> Result<ParquetChunkIterator, ParquetError> {
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::copy_from_slice(bytes))
        .map_err(|err| ParquetError::from(anyhow::Error::from(err)))?;

    let metadata = builder.metadata().clone();
    let reader = builder
        .build()
        .map_err(|err| ParquetError::from(anyhow::Error::from(err)))?;
    let schema = reader.schema().clone();

    build_iterator(
        Box::new(reader),
        schema,
        &metadata,
        config,
        entity_path_prefix.clone(),
    )
}

/// Construct a [`ParquetChunkIterator`] from a reader and config.
fn build_iterator(
    reader: Box<dyn Iterator<Item = Result<RecordBatch, arrow::error::ArrowError>>>,
    schema: Arc<arrow::datatypes::Schema>,
    parquet_metadata: &parquet::file::metadata::ParquetMetaData,
    config: &ParquetConfig,
    entity_path_prefix: EntityPath,
) -> Result<ParquetChunkIterator, ParquetError> {
    re_tracing::profile_function!();

    let timeline_infos: Vec<TimelineInfo> = if config.index_columns.is_empty() {
        vec![]
    } else {
        timeline::resolve_explicit_index_columns(&schema, &config.index_columns)?
    };

    let static_col_map: Vec<(usize, String)> = config
        .static_columns
        .iter()
        .filter_map(|name| {
            schema
                .fields()
                .iter()
                .position(|f| f.name() == name)
                .map(|idx| (idx, name.clone()))
        })
        .collect();

    let excluded: std::collections::HashSet<usize> = timeline_infos
        .iter()
        .map(|tl| tl.column_index)
        .chain(static_col_map.iter().map(|(idx, _)| *idx))
        .collect();

    let column_groups = compute_column_groups(
        &schema,
        &excluded,
        &entity_path_prefix,
        &config.column_grouping,
        &config.archetype_rules,
        &config.scalar_suffixes,
    );

    let metadata_chunk = build_metadata_chunk(parquet_metadata).map(Box::new);

    Ok(ParquetChunkIterator {
        phase: Phase::Metadata(metadata_chunk),
        reader,
        column_groups,
        timeline_infos,
        entity_path_prefix,
        schema,
        static_col_map,
        static_reference: None,
        row_offset: 0,
        pending: VecDeque::new(),
    })
}

// ---------------------------------------------------------------------------
// Iterator state machine
// ---------------------------------------------------------------------------

enum Phase {
    /// Yield the metadata chunk (if any), then transition to `DataBatches`.
    Metadata(Option<Box<Chunk>>),

    /// Read and process record batches.
    DataBatches,

    /// Terminal state.
    Done,
}

/// Pull-based iterator that yields [`Chunk`]s from a parquet file.
///
/// The iterator may yield `Err` for individual record batch failures.
/// Callers who want to continue despite errors should skip `Err` items.
pub(crate) struct ParquetChunkIterator {
    phase: Phase,
    reader: Box<dyn Iterator<Item = Result<RecordBatch, arrow::error::ArrowError>>>,
    column_groups: Vec<ColumnGroup>,
    timeline_infos: Vec<TimelineInfo>,
    entity_path_prefix: EntityPath,
    schema: Arc<arrow::datatypes::Schema>,

    /// Map from column index to column name for columns designated as static/timeless.
    static_col_map: Vec<(usize, String)>,

    /// First-row values for static columns, captured from the first batch.
    /// Used to verify consistency across subsequent batches.
    static_reference: Option<Vec<(String, Arc<dyn Array>)>>,

    /// Running row count across batches, used as offset for fallback `row_index` timeline.
    row_offset: i64,

    /// Chunks queued for yield by `next()`. Filled by `build_data_chunks` (one per
    /// column group per batch) and `build_finalization_chunks`. Bounded by the number
    /// of column groups, not file size.
    pending: VecDeque<Result<Chunk, ParquetError>>,
}

impl Iterator for ParquetChunkIterator {
    type Item = Result<Chunk, ParquetError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Some(item);
            }

            match self.phase {
                Phase::Metadata(ref mut meta) => {
                    let chunk = meta.take();
                    self.phase = Phase::DataBatches;
                    if let Some(c) = chunk {
                        return Some(Ok(*c));
                    }
                }

                Phase::DataBatches => match self.reader.next() {
                    Some(Ok(batch)) => {
                        if batch.num_rows() == 0 {
                            continue;
                        }

                        if let Err(err) = self.process_static_columns(&batch) {
                            self.phase = Phase::Done;
                            return Some(Err(err));
                        }

                        let timelines = self.build_timelines(&batch);
                        self.build_data_chunks(&batch, &timelines);

                        #[expect(clippy::cast_possible_wrap)]
                        {
                            self.row_offset += batch.num_rows() as i64;
                        }
                    }
                    Some(Err(err)) => {
                        return Some(Err(err.into()));
                    }
                    None => {
                        self.build_finalization_chunks();
                        self.phase = Phase::Done;
                    }
                },

                Phase::Done => return None,
            }
        }
    }
}

impl ParquetChunkIterator {
    /// Verify static columns are uniform and consistent across batches.
    fn process_static_columns(&mut self, batch: &RecordBatch) -> Result<(), ParquetError> {
        for (col_idx, col_name) in &self.static_col_map {
            let array = batch.column(*col_idx);
            verify_column_uniform(array.as_ref(), col_name)?;

            if let Some(ref refs) = self.static_reference {
                let ref_val = &refs
                    .iter()
                    .find(|(n, _)| n == col_name)
                    .expect("static reference should contain all static columns")
                    .1;
                let current_first = format_first_value(array.as_ref());
                let stored_first = format_first_value(ref_val.as_ref());
                if current_first != stored_first {
                    return Err(anyhow::anyhow!(
                        "Static column '{col_name}' changed between batches: \
                         '{stored_first}' → '{current_first}'"
                    )
                    .into());
                }
            }
        }

        if self.static_reference.is_none() && !self.static_col_map.is_empty() {
            self.static_reference = Some(
                self.static_col_map
                    .iter()
                    .map(|(col_idx, col_name)| {
                        (col_name.clone(), batch.column(*col_idx).slice(0, 1))
                    })
                    .collect(),
            );
        }

        Ok(())
    }

    /// Build timeline columns for a single batch.
    fn build_timelines(
        &self,
        batch: &RecordBatch,
    ) -> re_chunk::external::nohash_hasher::IntMap<re_chunk::TimelineName, TimeColumn> {
        let mut tls: re_chunk::external::nohash_hasher::IntMap<_, TimeColumn> = Default::default();
        for tl_info in &self.timeline_infos {
            let time_col = batch.column(tl_info.column_index);
            if let Some(times) =
                timeline::extract_time_values(time_col.as_ref(), tl_info.ns_multiplier)
            {
                let time_column = TimeColumn::new(Some(true), tl_info.timeline, times);
                tls.insert(*tl_info.timeline.name(), time_column);
            }
        }
        if tls.is_empty() {
            timeline::fallback_sequence_timeline(self.row_offset, batch.num_rows())
        } else {
            tls
        }
    }

    /// Build data chunks for each column group from a single batch.
    fn build_data_chunks(
        &mut self,
        batch: &RecordBatch,
        timelines: &re_chunk::external::nohash_hasher::IntMap<re_chunk::TimelineName, TimeColumn>,
    ) {
        let num_rows = batch.num_rows();

        for group in &self.column_groups {
            let components: re_chunk::ChunkComponents = group
                .entries
                .iter()
                .filter_map(|entry| match entry {
                    ColumnGroupEntry::Raw { col_idx, comp_name } => {
                        let field = &self.schema.fields()[*col_idx];
                        let array = batch.column(*col_idx).clone();
                        let list_array = wrap_in_fixed_size_list(field, array);
                        Some((
                            ComponentDescriptor::partial(comp_name.as_str()),
                            arrow::array::ListArray::from(list_array),
                        ))
                    }
                    ColumnGroupEntry::Archetype {
                        col_indices,
                        target,
                    } => {
                        let array = build_archetype_array(*target, batch, col_indices)?;
                        let inner_field =
                            Arc::new(Field::new("item", array.data_type().clone(), true));
                        let fsl = FixedSizeListArray::new(inner_field, 1, array, None);
                        Some((target.descriptor(), arrow::array::ListArray::from(fsl)))
                    }
                    ColumnGroupEntry::ScalarGroup {
                        col_indices,
                        names: _,
                    } => {
                        let n = col_indices.len();
                        let columns: Vec<Vec<f64>> = col_indices
                            .iter()
                            .map(|&idx| read_f64_column(batch.column(idx).as_ref()))
                            .collect::<Option<Vec<_>>>()?;

                        let mut values = Vec::with_capacity(num_rows * n);
                        for i in 0..num_rows {
                            for col in &columns {
                                values.push(col[i]);
                            }
                        }

                        let float_array = Float64Array::from(values);
                        let inner_field = Arc::new(Field::new("item", DataType::Float64, false));
                        let n_i32 = i32::try_from(n).expect("scalar suffix group too large");
                        let fsl = FixedSizeListArray::new(
                            inner_field,
                            n_i32,
                            Arc::new(float_array),
                            None,
                        );
                        Some((
                            re_sdk_types::archetypes::Scalars::descriptor_scalars(),
                            arrow::array::ListArray::from(fsl),
                        ))
                    }
                })
                .collect();

            match Chunk::from_auto_row_ids(
                ChunkId::new(),
                group.entity_path.clone(),
                timelines.clone(),
                components,
            ) {
                Ok(chunk) => {
                    self.pending.push_back(Ok(chunk));
                }
                Err(err) => {
                    self.pending.push_back(Err(anyhow::anyhow!(
                        "Failed to build chunk from Parquet batch: {err}"
                    )
                    .into()));
                }
            }
        }
    }

    /// Build finalization chunks: static columns + scalar name components.
    fn build_finalization_chunks(&mut self) {
        // Static columns as a single timeless chunk.
        if let Some(ref refs) = self.static_reference {
            let components: re_chunk::ChunkComponents = refs
                .iter()
                .map(|(name, array)| {
                    let field = Field::new(name.as_str(), array.data_type().clone(), true);
                    let list_array = wrap_in_fixed_size_list(&field, array.clone());
                    (
                        ComponentDescriptor::partial(name.as_str()),
                        arrow::array::ListArray::from(list_array),
                    )
                })
                .collect();

            match Chunk::from_auto_row_ids(
                ChunkId::new(),
                self.entity_path_prefix.clone(),
                Default::default(),
                components,
            ) {
                Ok(chunk) => {
                    self.pending.push_back(Ok(chunk));
                }
                Err(err) => {
                    self.pending
                        .push_back(Err(
                            anyhow::anyhow!("Failed to build static chunk: {err}").into()
                        ));
                }
            }
        }

        // Static Name components for scalar groups.
        for group in &self.column_groups {
            for entry in &group.entries {
                if let ColumnGroupEntry::ScalarGroup { names, .. } = entry {
                    let names_array = arrow::array::StringArray::from(names.clone());
                    let inner_field = Arc::new(Field::new("item", DataType::Utf8, false));
                    let n = i32::try_from(names.len()).expect("scalar suffix group too large");
                    let fsl = FixedSizeListArray::new(inner_field, n, Arc::new(names_array), None);
                    let components: re_chunk::ChunkComponents = std::iter::once((
                        re_sdk_types::archetypes::SeriesLines::descriptor_names(),
                        arrow::array::ListArray::from(fsl),
                    ))
                    .collect();

                    match Chunk::from_auto_row_ids(
                        ChunkId::new(),
                        group.entity_path.clone(),
                        Default::default(),
                        components,
                    ) {
                        Ok(chunk) => {
                            self.pending.push_back(Ok(chunk));
                        }
                        Err(err) => {
                            self.pending.push_back(Err(anyhow::anyhow!(
                                "Failed to build name chunk: {err}"
                            )
                            .into()));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata chunk
// ---------------------------------------------------------------------------

/// Build a static chunk from parquet file-level key-value metadata.
fn build_metadata_chunk(metadata: &parquet::file::metadata::ParquetMetaData) -> Option<Chunk> {
    let kv_metadata = metadata.file_metadata().key_value_metadata()?;

    if kv_metadata.is_empty() {
        return None;
    }

    let pairs: Vec<datatypes::Utf8Pair> = kv_metadata
        .iter()
        .map(|kv| datatypes::Utf8Pair {
            first: kv.key.clone().into(),
            second: kv.value.clone().unwrap_or_default().into(),
        })
        .collect();

    let kv_component = re_sdk_types::components::KeyValuePairs(pairs);

    let batch = kv_component
        .try_serialized(ComponentDescriptor {
            archetype: Some(PARQUET_METADATA_ARCHETYPE.into()),
            component: "file_metadata".into(),
            component_type: Some(re_sdk_types::components::KeyValuePairs::name()),
        })
        .ok()?;

    Chunk::builder(EntityPath::properties())
        .with_serialized_batches(RowId::new(), TimePoint::STATIC, [batch])
        .build()
        .ok()
}

// ---------------------------------------------------------------------------
// Arrow utilities
// ---------------------------------------------------------------------------

/// Build a `FixedSizeList(N, Float32)` array from `N` scalar columns.
fn build_archetype_array(
    target: MappedComponent,
    batch: &RecordBatch,
    col_indices: &[usize],
) -> Option<Arc<dyn Array>> {
    let num_rows = batch.num_rows();
    let n = target.element_count();
    re_log::debug_assert_eq!(col_indices.len(), n);

    let columns: Vec<Vec<f32>> = col_indices
        .iter()
        .map(|&idx| read_f32_column(batch.column(idx).as_ref()))
        .collect::<Option<Vec<_>>>()?;

    let mut values = Vec::with_capacity(num_rows * n);
    for i in 0..num_rows {
        for col in &columns {
            values.push(col[i]);
        }
    }

    let float_array = Float32Array::from(values);
    let inner_field = Arc::new(Field::new("item", DataType::Float32, false));
    let n_i32 = i32::try_from(n).expect("archetype element count too large");
    Some(Arc::new(FixedSizeListArray::new(
        inner_field,
        n_i32,
        Arc::new(float_array),
        None,
    )))
}

/// Convert a numeric Arrow array to `Vec<f64>` via arrow cast.
fn read_f64_column(array: &dyn Array) -> Option<Vec<f64>> {
    let casted = arrow::compute::cast(array, &DataType::Float64)
        .map_err(|_err| {
            re_log::warn_once!(
                "Unsupported column type for scalar mapping: {:?}",
                array.data_type()
            );
        })
        .ok()?;
    let arr = casted.as_any().downcast_ref::<Float64Array>()?;
    Some(arr.values().iter().copied().collect())
}

/// Convert a numeric Arrow array to `Vec<f32>` via arrow cast.
fn read_f32_column(array: &dyn Array) -> Option<Vec<f32>> {
    let casted = arrow::compute::cast(array, &DataType::Float32)
        .map_err(|_err| {
            re_log::warn_once!(
                "Unsupported column type for archetype mapping: {:?}",
                array.data_type()
            );
        })
        .ok()?;
    let arr = casted.as_any().downcast_ref::<Float32Array>()?;
    Some(arr.values().iter().copied().collect())
}

/// Verify that every value in `array` is identical.
fn verify_column_uniform(array: &dyn Array, col_name: &str) -> Result<(), ParquetError> {
    if array.len() <= 1 {
        return Ok(());
    }
    if !is_array_uniform(array) {
        return Err(
            anyhow::anyhow!("Static column '{col_name}' contains non-uniform values").into(),
        );
    }
    Ok(())
}

/// Check whether all elements in an Arrow array are equal to the first element.
fn is_array_uniform(array: &dyn Array) -> bool {
    let len = array.len();
    if len <= 1 {
        return true;
    }
    // slice returns ArrayRef which implements Datum (needed by cmp::eq).
    let all = array.slice(0, len);
    let first = arrow::array::Scalar::new(array.slice(0, 1));
    if let Ok(bools) = arrow::compute::kernels::cmp::eq(&all, &first) {
        bools.true_count() == len
    } else {
        re_log::warn_once!(
            "Cannot verify uniformity for column type {:?}, assuming uniform",
            array.data_type()
        );
        true
    }
}

/// Format the first element of an array as a string for cross-batch comparison.
fn format_first_value(array: &dyn Array) -> String {
    if array.is_empty() {
        return String::new();
    }

    macro_rules! fmt_primitive {
        ($arr_ty:ty) => {
            if let Some(arr) = array.as_any().downcast_ref::<$arr_ty>() {
                return format!("{}", arr.value(0));
            }
        };
    }
    fmt_primitive!(Float64Array);
    fmt_primitive!(Float32Array);
    fmt_primitive!(arrow::array::Int64Array);
    fmt_primitive!(arrow::array::Int32Array);

    if let Some(arr) = array.as_any().downcast_ref::<arrow::array::StringArray>() {
        return arr.value(0).to_owned();
    }
    if let Some(arr) = array
        .as_any()
        .downcast_ref::<arrow::array::LargeStringArray>()
    {
        return arr.value(0).to_owned();
    }

    format!("{array:?}")
}

/// Wrap each element of an array into a `FixedSizeList` of size 1.
fn wrap_in_fixed_size_list(field: &Field, array: Arc<dyn Array>) -> FixedSizeListArray {
    let inner_field = Arc::new(Field::new(
        "item",
        field.data_type().clone(),
        field.is_nullable(),
    ));
    FixedSizeListArray::new(inner_field, 1, array, None)
}
