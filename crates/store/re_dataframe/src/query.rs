use std::sync::OnceLock;
use std::{collections::BTreeSet, iter::repeat_n};

use arrow::array::{
    Array as _, ArrayData, ArrayRef as ArrowArrayRef, BooleanArray as ArrowBooleanArray,
    MutableArrayData, PrimitiveArray as ArrowPrimitiveArray, RecordBatch as ArrowRecordBatch,
    RecordBatchOptions, make_array,
};
use arrow::buffer::{NullBuffer, ScalarBuffer as ArrowScalarBuffer};
use arrow::datatypes::{
    DataType as ArrowDataType, Fields as ArrowFields, Schema as ArrowSchema,
    SchemaRef as ArrowSchemaRef,
};
use itertools::{Either, Itertools as _};
use nohash_hasher::{IntMap, IntSet};
use re_arrow_util::{ArrowArrayDowncastRef as _, into_arrow_ref};
use re_chunk::external::arrow::array::ArrayRef;
use re_chunk::{
    Chunk, ComponentIdentifier, EntityPath, RangeQuery, RowId, TimeInt, TimelineName,
    UnitChunkShared,
};
use re_chunk_store::{
    ChunkStore, ChunkTrackingMode, ColumnDescriptor, ComponentColumnDescriptor, Index,
    IndexColumnDescriptor, IndexValue, QueryExpression, SparseFillStrategy,
};
use re_log::{debug_assert, debug_assert_eq, debug_panic};
use re_log_types::AbsoluteTimeRange;
use re_query::{QueryCache, StorageEngineLike};
use re_sorbet::{
    ChunkColumnDescriptors, ColumnSelector, RowIdColumnDescriptor, TimeColumnSelector,
};
use re_span::Span;
use re_types_core::arrow_helpers::as_array_ref;
use re_types_core::{Loggable as _, SerializedComponentColumn, archetypes};

// ---

/// Streaming-join state for a single component column on a single row.
#[derive(Debug)]
struct StreamingJoinStateEntry<'a> {
    /// Which `Chunk` is this?
    chunk: &'a Chunk,

    /// How far are we into this `Chunk`?
    cursor: u64,

    /// What's the `RowId` at the current cursor?
    row_id: RowId,
}

/// Streaming-join state for a single component column on a single row.
///
/// Possibly retrofilled, see [`QueryExpression::sparse_fill_strategy`].
#[derive(Debug)]
enum StreamingJoinState<'a> {
    /// Incoming data for the current iteration.
    StreamingJoinState(StreamingJoinStateEntry<'a>),

    /// Data retrofilled through an extra query.
    ///
    /// See [`QueryExpression::sparse_fill_strategy`].
    Retrofilled(UnitChunkShared),
}

/// Per-row index data resolved by [`QueryHandle::_resolve_one_row`]: the max value
/// seen on each timeline for this row.
type ResolvedRow = IntMap<TimelineName, (TimeInt, ArrowScalarBuffer<i64>)>;

/// Output of [`QueryHandle::next_n_rows`] / [`QueryHandle::next_n_rows_async`].
///
/// `columns` always has length equal to `schema().fields().len()`. `num_rows` is the number
/// of rows actually appended (≤ requested `max_rows`; 0 means the query is exhausted).
#[derive(Debug)]
pub struct NextNRowsOutput {
    pub columns: Vec<ArrowArrayRef>,
    pub num_rows: usize,
}

/// Minimum bulk-emit run length below which `_next_n_rows` falls through to the
/// per-row streaming-join path. Avoids paying bulk-machinery overhead on very
/// short eligible runs.
const BULK_MIN_RUN: usize = 4;

/// One step in the deferred-replay finalizer inside `_next_n_rows`.
///
/// Each output column accumulates a list of [`ColumnExtend`] entries during the
/// row-resolution loop and replays them via `MutableArrayData` once the batch is
/// fully sized, allowing single-row pushes to coalesce into long abutting runs.
#[derive(Debug, Clone, Copy)]
enum ColumnExtend {
    /// Append a contiguous row run by copying `rows` from one of the column's
    /// distinct source arrays (`SelectedEmitter::Source::sources[source_idx]`).
    Range {
        /// Index into `SelectedEmitter::Source::sources` — picks which previously
        /// registered source array this run reads from. Source arrays are
        /// deduplicated by their underlying chunk pointer at registration time
        /// (see `SelectedEmitter::ensure_source`), so multiple runs from the
        /// same chunk share a single `source_idx`.
        source_idx: usize,

        /// Row range to copy out of `sources[source_idx]` (start row, inclusive,
        /// for `len` rows).
        rows: Span<usize>,
    },

    /// Append `len` null rows.
    Nulls { len: usize },
}

/// Per-output-column emission state used by [`QueryHandle::_next_n_rows`].
///
/// `Source` columns accumulate [`ColumnExtend`] entries that reference shared
/// source arrays (deduplicated by chunk pointer), then replay them through
/// `MutableArrayData` once the batch is fully sized. `Time` columns push i64
/// values + a validity mask directly.
enum SelectedEmitter {
    Source {
        sources: Vec<ArrayData>,

        source_ids: Vec<*const Chunk>, // Used over `ChunkId` for speed.

        /// `source_bytes_per_row[i]` is the estimated bytes-per-row for `sources[i]`, computed
        /// as `get_array_memory_size() / len()` at registration time. Cheap (one
        /// division per distinct source per batch) and lets the walk amortize the byte
        /// budget without inspecting `MutableArrayData` until freeze.
        source_bytes_per_row: Vec<usize>,

        extends: Vec<ColumnExtend>,
    },
    Time {
        values: Vec<i64>,
        valid: Vec<bool>,
    },
}

impl SelectedEmitter {
    fn ensure_source(
        sources: &mut Vec<ArrayData>,
        source_ids: &mut Vec<*const Chunk>,
        source_bytes_per_row: &mut Vec<usize>,
        id: *const Chunk,
        data: impl FnOnce() -> ArrayData,
    ) -> usize {
        if let Some(idx) = source_ids.iter().position(|x| *x == id) {
            idx
        } else {
            let idx = sources.len();
            let d = data();
            let bpr = d.get_array_memory_size().checked_div(d.len()).unwrap_or(0);
            sources.push(d);
            source_ids.push(id);
            source_bytes_per_row.push(bpr);
            idx
        }
    }

    /// Append a contiguous row run drawn from `sources[source_idx]`,
    /// merging with a trailing abutting `Range` from the same source.
    /// Coalescing turns long runs of single-row extends into a handful
    /// of multi-row extends, shrinking the replay loop in the finalizer.
    fn push_run(extends: &mut Vec<ColumnExtend>, source_idx: usize, rows: Span<usize>) {
        if rows.len == 0 {
            return;
        }
        if let Some(ColumnExtend::Range {
            source_idx: prev_src,
            rows: prev_rows,
        }) = extends.last_mut()
            && *prev_src == source_idx
            && prev_rows.end() == rows.start
        {
            prev_rows.len += rows.len;
            return;
        }
        extends.push(ColumnExtend::Range { source_idx, rows });
    }

    /// Append `len` null rows, merging with a trailing `Nulls` entry.
    fn push_nulls(extends: &mut Vec<ColumnExtend>, len: usize) {
        if len == 0 {
            return;
        }
        if let Some(ColumnExtend::Nulls { len: prev_len }) = extends.last_mut() {
            *prev_len += len;
            return;
        }
        extends.push(ColumnExtend::Nulls { len });
    }
}

/// Per-view-column classification produced by [`QueryHandle::try_bulk_emit_run`].
///
/// At each `cur_row` the bulk path classifies how every non-empty view column
/// can contribute to the upcoming run. The run length is the `min` of
/// per-column lengths; if any column is not eligible the bulk attempt bails.
#[derive(Clone, Copy, Debug)]
enum ColumnRunClass {
    /// Slice a contiguous run from this view column's active chunk:
    /// the column emits `rows.len` rows from `chunks[chunk_idx]` starting
    /// at `rows.start` (zero-copy Arrow slice).
    Slice {
        /// Index of the active chunk in
        /// [`QueryHandleState::view_chunks`]`[view_idx]`.
        chunk_idx: usize,

        /// Row range to read from the active chunk.
        /// `rows.start` always equals `cur_row - chunk.dense_uiv_span.start`
        /// for bulk-eligible (dense + unique) chunks. `rows.len` is the
        /// chunk's contribution to this run before exhaustion; the
        /// global bulk run length is the `min` across all columns.
        rows: Span<usize>,
    },

    /// The column has no chunk covering `cur_row`; emit `len` null rows
    /// for it. `len` extends until the next chunk in this column (or
    /// to the end of `unique_index_values` if there is no next chunk).
    Null { len: usize },
}

// TODO(cmc): (no specific order) (should we make issues for these?)
// * [x] basic thing working
// * [x] custom selection
// * [x] support for overlaps (slow)
// * [x] pagination (any solution, even a slow one)
// * [x] pov support
// * [x] latestat sparse-filling
// * [x] sampling support
// * [x] clears
// * [x] pagination (fast)
// * [x] take kernel duplicates all memory
// * [x] dedupe-latest without allocs/copies
// * [ ] allocate null arrays once
// * [ ] overlaps (less dumb)
// * [ ] selector-based `filtered_index`
// * [ ] configurable cache bypass

/// A handle to a dataframe query, ready to be executed.
///
/// Cheaply created via `QueryEngine::query`.
///
/// See [`QueryHandle::next_row`] or [`QueryHandle::into_iter`].
pub struct QueryHandle<E: StorageEngineLike> {
    /// Handle to the `QueryEngine`.
    pub(crate) engine: E,

    /// The original query expression used to instantiate this handle.
    pub(crate) query: QueryExpression,

    /// Immutable view metadata. Lazily computed on first use.
    ///
    /// It is important that handles stay cheap to create.
    state: OnceLock<QueryHandleState>,

    /// Mutable iteration state. Lazily initialized on the first `&mut self` call.
    ///
    /// Layout mirrors [`QueryHandleState::view_chunks`].
    iter_state: Option<IterState>,
}

/// Immutable per-chunk metadata used by the streaming-join walk.
///
/// `time_min`/`time_max` are cached at init from the chunk's time column for the query's
/// `filtered_index`, used for range-based pruning and as the sort key for early-break iteration.
struct ChunkBundle {
    chunk: Chunk,
    time_min: i64,
    time_max: i64,

    /// True if this chunk's `filtered_index` time range does not overlap any other
    /// chunk's time range in the same view column. Filled in [`QueryHandle::init_`]
    /// after view chunks are sorted by `time_min`.
    is_disjoint_in_column: bool,

    /// True if this chunk's `filtered_index` times are strictly increasing.
    times_unique: bool,

    /// `Some(span)` iff this chunk's rows map 1:1 to
    /// `unique_index_values[span.range()]`. `None` when the chunk lacks the
    /// `filtered_index` timeline, has duplicate timestamps that fold into
    /// fewer `unique_index_values` entries, or has gaps where another column's
    /// chunks add intervening index values.
    dense_uiv_span: Option<Span<usize>>,
}

impl ChunkBundle {
    fn new(chunk: Chunk, filtered_index: Option<&Index>) -> Self {
        let (time_min, time_max) = filtered_index
            .and_then(|idx| chunk.timelines().get(idx))
            .map_or((i64::MIN, i64::MAX), |tc| {
                let r = tc.time_range();
                (r.min().as_i64(), r.max().as_i64())
            });
        Self {
            chunk,
            time_min,
            time_max,
            is_disjoint_in_column: false,
            times_unique: false,
            dense_uiv_span: None,
        }
    }
}

/// Mutable per-chunk iteration state for the streaming-join walk.
///
/// `cursor` tracks the current row inside the corresponding [`ChunkBundle::chunk`].
/// `exhausted` is set once the cursor moves past `chunk.num_rows()` or once
/// `cur_index_value` exceeds `time_max`, allowing later rows to skip the chunk entirely.
#[derive(Debug, Default, Clone, Copy)]
struct ChunkIterCursor {
    cursor: u64,
    exhausted: bool,
}

/// Mutable iteration state. Lazily initialized on the first `&mut self` iteration call,
/// after the immutable [`QueryHandleState`] has been built.
///
/// Lives outside `OnceLock` so that the public iteration API can take `&mut self` and
/// mutate it through plain Rust references (no `Cell`/atomics needed).
struct IterState {
    /// Current row index: the position of the iterator. For [`QueryHandle::next_row`].
    ///
    /// This represents the number of rows that the caller has iterated on; unrelated to
    /// the per-chunk cursors. The corresponding index value is
    /// [`QueryHandleState::unique_index_values`]`[cur_row]`.
    cur_row: u64,

    /// Per-view, per-chunk cursors. Layout mirrors [`QueryHandleState::view_chunks`]
    /// exactly: `view_chunks[view_idx][chunk_idx]` corresponds to
    /// `state.view_chunks[view_idx][chunk_idx]`.
    view_chunks: Vec<Vec<ChunkIterCursor>>,

    /// Total number of rows emitted via [`QueryHandle::try_bulk_emit_run`].
    ///
    /// Exposed for tests via `QueryHandle::bulk_emitted_rows` so they can assert
    /// the bulk fast path was actually taken (a regression that silently disabled
    /// it would otherwise still produce correct output via the per-row fallback).
    bulk_emitted_rows: u64,
}

impl IterState {
    fn new(state: &QueryHandleState) -> Self {
        Self {
            cur_row: 0,
            view_chunks: state
                .view_chunks
                .iter()
                .map(|chunks| vec![ChunkIterCursor::default(); chunks.len()])
                .collect(),
            bulk_emitted_rows: 0,
        }
    }
}

/// Internal private state. Lazily computed.
struct QueryHandleState {
    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression::view_contents`].
    view_contents: ChunkColumnDescriptors,

    /// Describes the columns specifically selected to be returned from this view.
    ///
    /// All returned rows will have an Arrow schema that matches this selection.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// The extra `usize` is the index in [`QueryHandleState::view_contents`] that this selection
    /// points to.
    ///
    /// See also [`QueryHandleState::arrow_schema`].
    selected_contents: Vec<(usize, ColumnDescriptor)>,

    /// This keeps track of the static data associated with each entry in `selected_contents`, if any.
    ///
    /// This is queried only once during init, and will override all cells that follow.
    ///
    /// `selected_contents`: [`QueryHandleState::selected_contents`]
    selected_static_values: Vec<Option<UnitChunkShared>>,

    /// The actual index filter in use, since the user-specified one is optional.
    ///
    /// This just defaults to `Index::default()` if the user hasn't specified any: the actual
    /// value is irrelevant since this means we are only concerned with static data anyway.
    filtered_index: Index,

    /// The Arrow schema that corresponds to the `selected_contents`.
    ///
    /// All returned rows will have this schema.
    arrow_schema: ArrowSchemaRef,

    /// All the [`Chunk`]s included in the view contents, with their cached time ranges.
    ///
    /// These are already sorted, densified, vertically sliced, and [latest-deduped] according
    /// to the query. Per-chunk iteration cursors live in [`IterState::view_chunks`] with
    /// matching layout (chunks are allowed to overlap, so iteration may rebound between two
    /// or more chunks).
    ///
    /// This vector's entries correspond to those in [`QueryHandleState::view_contents`].
    /// Note: time and column entries don't have chunks -- inner vectors will be empty.
    ///
    /// [latest-deduped]: [`Chunk::deduped_latest_on_index`]
    //
    // NOTE: Reminder: we have to query everything in the _view_, irrelevant of the current selection.
    view_chunks: Vec<Vec<ChunkBundle>>,

    /// All unique index values that can possibly be returned by this query.
    ///
    /// Guaranteed ascendingly sorted and deduped.
    ///
    /// See also [`IterState::cur_row`].
    unique_index_values: Vec<IndexValue>,
}

impl<E: StorageEngineLike> QueryHandle<E> {
    pub(crate) fn new(engine: E, query: QueryExpression) -> Self {
        Self {
            engine,
            query,
            state: Default::default(),
            iter_state: None,
        }
    }
}

/// Apply a user-supplied `selection` against a view-contents schema and
/// return the resolved column list.
///
/// Each entry is `(idx, descr)` where `idx` is the column's position in
/// `view_contents` if the selection hit a real column, or `usize::MAX` if
/// the selector did not match (the corresponding entry will emit all-null
/// values at query time).
///
/// Used by both [`QueryHandle::init_`] and [`crate::QueryEngine::selected_schema_for_query`].
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn compute_user_selection(
    view_contents: &[ColumnDescriptor],
    selection: &[ColumnSelector],
) -> Vec<(usize, ColumnDescriptor)> {
    selection
        .iter()
        .map(|column| match column {
            ColumnSelector::RowId => view_contents
                .iter()
                .enumerate()
                .find_map(|(idx, view_column)| {
                    if let ColumnDescriptor::RowId(descr) = view_column {
                        Some((idx, ColumnDescriptor::RowId(descr.clone())))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    (
                        usize::MAX,
                        ColumnDescriptor::RowId(RowIdColumnDescriptor::from_sorted(false)),
                    )
                }),

            ColumnSelector::Time(selected_column) => {
                let TimeColumnSelector {
                    timeline: selected_timeline,
                } = selected_column;

                view_contents
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, view_column)| {
                        if let ColumnDescriptor::Time(view_descr) = view_column {
                            Some((idx, view_descr))
                        } else {
                            None
                        }
                    })
                    .find(|(_idx, view_descr)| *view_descr.timeline().name() == *selected_timeline)
                    .map_or_else(
                        || {
                            (
                                usize::MAX,
                                ColumnDescriptor::Time(IndexColumnDescriptor::new_null(
                                    *selected_timeline,
                                )),
                            )
                        },
                        |(idx, view_descr)| (idx, ColumnDescriptor::Time(view_descr.clone())),
                    )
            }

            ColumnSelector::Component(selected_column) => view_contents
                .iter()
                .enumerate()
                .filter_map(|(idx, view_column)| {
                    if let ColumnDescriptor::Component(view_descr) = view_column {
                        Some((idx, view_descr))
                    } else {
                        None
                    }
                })
                .find(|(_idx, view_descr)| view_descr.matches(selected_column))
                .map_or_else(
                    || {
                        (
                            usize::MAX,
                            ColumnDescriptor::Component(ComponentColumnDescriptor {
                                entity_path: selected_column.entity_path.clone(),
                                archetype: None,
                                component: selected_column.component.as_str().into(),
                                component_type: None,
                                store_datatype: ArrowDataType::Null,
                                is_static: false,
                                is_tombstone: false,
                                is_semantically_empty: false,
                            }),
                        )
                    },
                    |(idx, view_descr)| (idx, ColumnDescriptor::Component(view_descr.clone())),
                ),
        })
        .collect_vec()
}

impl<E: StorageEngineLike> QueryHandle<E> {
    /// Lazily initialize internal private state.
    ///
    /// It is important that query handles stay cheap to create.
    fn init(&self) -> &QueryHandleState {
        self.engine.with(|store, cache| {
            self.state
                .get_or_init(|| Self::init_(&self.query, store, cache))
        })
    }

    /// Trigger lazy initialization of both `state` and `iter_state`, then return split
    /// borrows into the immutable view metadata and the mutable iteration state.
    ///
    /// All `&mut self` iteration entry points funnel through this helper so the rest of
    /// the implementation can work against `(&QueryHandleState, &mut IterState)` directly,
    /// without further interior mutability.
    fn init_iter(&mut self) -> (&QueryHandleState, &mut IterState) {
        // First trigger immutable lazy init so the OnceLock is populated.
        let _ = self.init();
        // Now split-borrow `state` (immut, via OnceLock::get) from `iter_state` (mut).
        let Self {
            state, iter_state, ..
        } = self;
        let state = state.get().expect("state was just initialized by init()");
        let iter_state = iter_state.get_or_insert_with(|| IterState::new(state));
        (state, iter_state)
    }

    // NOTE: This is split in its own method otherwise it completely breaks `rustfmt`.
    #[tracing::instrument(level = "debug", skip_all)]
    fn init_(query: &QueryExpression, store: &ChunkStore, cache: &QueryCache) -> QueryHandleState {
        re_tracing::profile_scope!("QueryHandle::init");

        // The timeline doesn't matter if we're running in static-only mode.
        let filtered_index = query
            .filtered_index
            .unwrap_or_else(|| TimelineName::new(""));

        // 1. Compute the schema for the query.
        let view_contents_schema = store.schema_for_query(query);
        let view_contents = view_contents_schema.indices_and_components();

        // 2. Compute the schema of the selected contents.
        //
        // The caller might have selected columns that do not exist in the view: they should
        // still appear in the results.
        let selected_contents: Vec<(_, _)> = if let Some(selection) = query.selection.as_ref() {
            compute_user_selection(&view_contents, selection)
        } else {
            view_contents.clone().into_iter().enumerate().collect()
        };

        // 3. Compute the Arrow schema of the selected components.
        //
        // Every result returned using this `QueryHandle` will match this schema exactly.
        let arrow_schema = ArrowSchemaRef::from(ArrowSchema::new_with_metadata(
            selected_contents
                .iter()
                .map(|(_, descr)| descr.to_arrow_field(re_sorbet::BatchType::Dataframe))
                .collect::<ArrowFields>(),
            Default::default(),
        ));

        // 4. Perform the query and keep track of all the relevant chunks.
        let range_query = {
            let index_range = if query.filtered_index.is_none() {
                AbsoluteTimeRange::EMPTY // static-only
            } else if let Some(using_index_values) = query.using_index_values.as_ref() {
                Option::zip(using_index_values.first(), using_index_values.last())
                    .map_or(AbsoluteTimeRange::EMPTY, |(start, end)| {
                        AbsoluteTimeRange::new(*start, *end)
                    })
            } else {
                query
                    .filtered_index_range
                    .unwrap_or(AbsoluteTimeRange::EVERYTHING)
            };

            RangeQuery::new(filtered_index, index_range)
                .keep_extra_timelines(true) // we want all the timelines we can get!
                .keep_extra_components(false)
        };
        let (view_pov_chunks_idx, mut view_chunks) =
            Self::fetch_view_chunks(query, store, cache, &range_query, &view_contents);

        // 5. Collect all relevant clear chunks and update the view accordingly.
        //
        // We'll turn the clears into actual empty arrays of the expected component type.
        {
            re_tracing::profile_scope!("clear_chunks");

            let clear_chunks =
                Self::fetch_clear_chunks(query, store, cache, &range_query, &view_contents);
            for (view_idx, chunks) in view_chunks.iter_mut().enumerate() {
                let Some(ColumnDescriptor::Component(descr)) = view_contents.get(view_idx) else {
                    continue;
                };

                descr.sanity_check();

                // NOTE: It would be tempting to concatenate all these individual clear chunks into one
                // single big chunk, but that'd be a mistake: 1) it's costly to do so but more
                // importantly 2) that would lead to likely very large chunk overlap, which is very bad
                // for business.
                if let Some(clear_chunks) = clear_chunks.get(&descr.entity_path) {
                    chunks.extend(clear_chunks.iter().map(|chunk| {
                        let child_datatype = match &descr.store_datatype {
                            ArrowDataType::List(field) | ArrowDataType::LargeList(field) => {
                                field.data_type().clone()
                            }
                            ArrowDataType::Dictionary(_, datatype) => (**datatype).clone(),
                            datatype => datatype.clone(),
                        };

                        let mut chunk = chunk.clone();
                        // Only way this could fail is if the number of rows did not match.
                        #[expect(clippy::unwrap_used)]
                        chunk
                            .add_component(SerializedComponentColumn::new(
                                re_arrow_util::new_list_array_of_empties(
                                    &child_datatype,
                                    chunk.num_rows(),
                                ),
                                re_types_core::ComponentDescriptor {
                                    component_type: descr.component_type,
                                    archetype: descr.archetype,
                                    component: descr.component,
                                },
                            ))
                            .unwrap();

                        ChunkBundle::new(chunk, Some(&filtered_index))
                    }));
                }
            }
        }

        // 5b. Sort each view's chunks by `time_min` ascending.
        //
        // The streaming-join walk relies on this ordering to break out of the per-row inner
        // loop as soon as it sees a chunk whose `time_min > cur_index_value`. Order doesn't
        // affect row output because the streaming-join already takes max-RowId across
        // overlapping chunks for any given index value.
        for chunks in &mut view_chunks {
            chunks.sort_by_key(|cc| cc.time_min);
        }

        // 6. Collect all unique index values.
        //
        // Used to achieve ~O(log(n)) pagination.
        let unique_index_values = if query.filtered_index.is_none() {
            vec![TimeInt::STATIC]
        } else if let Some(using_index_values) = query.using_index_values.as_ref() {
            using_index_values
                .iter()
                .filter(|index_value| !index_value.is_static())
                .copied()
                .collect_vec()
        } else {
            re_tracing::profile_scope!("index_values");

            let mut view_chunks = view_chunks.iter();
            let view_chunks = if let Some(view_pov_chunks_idx) = view_pov_chunks_idx {
                Either::Left(view_chunks.nth(view_pov_chunks_idx).into_iter())
            } else {
                Either::Right(view_chunks)
            };

            let mut all_unique_index_values: BTreeSet<TimeInt> = view_chunks
                .flat_map(|chunks| {
                    chunks.iter().filter_map(|cc| {
                        cc.chunk
                            .timelines()
                            .get(&filtered_index)
                            .map(|time_column| time_column.times())
                    })
                })
                .flatten()
                .collect();

            if let Some(filtered_index_values) = query.filtered_index_values.as_ref() {
                all_unique_index_values.retain(|time| filtered_index_values.contains(time));
            }

            all_unique_index_values
                .into_iter()
                .filter(|index_value| !index_value.is_static())
                .collect_vec()
        };

        // 6b. Fill per-chunk bulk-emit metadata, now that both `view_chunks` (sorted)
        //     and `unique_index_values` are known.
        Self::fill_bulk_metadata(&mut view_chunks, &unique_index_values, &filtered_index);

        let selected_static_values = {
            re_tracing::profile_scope!("static_values");

            selected_contents
                .iter()
                .map(|(_view_idx, descr)| match descr {
                    ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => None,
                    ColumnDescriptor::Component(descr) => {
                        descr.sanity_check();

                        let query =
                            re_chunk::LatestAtQuery::new(TimelineName::new(""), TimeInt::STATIC);

                        let results = cache.latest_at(
                            ChunkTrackingMode::Report,
                            &query,
                            &descr.entity_path,
                            [descr.component],
                        );

                        results.components.into_values().next()
                    }
                })
                .collect_vec()
        };

        for (_, descr) in &selected_contents {
            descr.sanity_check();
        }

        QueryHandleState {
            view_contents: view_contents_schema,
            selected_contents,
            selected_static_values,
            filtered_index,
            arrow_schema,
            view_chunks,
            unique_index_values,
        }
    }

    /// Fills [`ChunkBundle`] bulk-emit metadata for every view-column chunk.
    ///
    /// Must be called after the view's chunks are sorted by `time_min` (so neighbor
    /// comparisons make sense) and after `unique_index_values` has been computed.
    #[tracing::instrument(level = "trace", skip_all)]
    fn fill_bulk_metadata(
        view_chunks: &mut [Vec<ChunkBundle>],
        unique_index_values: &[IndexValue],
        filtered_index: &Index,
    ) {
        re_tracing::profile_function!();

        // Map each unique_index_value to its position via the assumption that
        // `unique_index_values` is sorted ascending and dedup'd.
        for chunks in view_chunks.iter_mut() {
            // Step 1: per-chunk `is_disjoint_in_column`.
            //
            // Chunks are sorted by `time_min` ascending, but `time_max` can
            // extend arbitrarily past the next chunk's `time_min` (e.g. a
            // long-lived chunk early in the list overlapping every later
            // chunk). Track the running max of `time_max` across all prior
            // chunks to catch that case; the next-neighbor check is enough
            // for the forward direction since chunks are sorted on `time_min`
            // (if the next chunk's `time_min` is past current `time_max`,
            // every later chunk's `time_min` is too).
            let n = chunks.len();
            let mut max_prev_time_max = i64::MIN;
            for i in 0..n {
                let lo = chunks[i].time_min;
                let hi = chunks[i].time_max;
                let prev_ok = max_prev_time_max < lo;
                let next_ok = i + 1 == n || hi < chunks[i + 1].time_min;
                chunks[i].is_disjoint_in_column = prev_ok && next_ok;
                max_prev_time_max = max_prev_time_max.max(hi);
            }

            // Step 2: per-chunk `times_unique` and `dense_uiv_span`.
            for cc in chunks.iter_mut() {
                let Some(time_column) = cc.chunk.timelines().get(filtered_index) else {
                    // Chunk doesn't carry the filtered_index timeline.
                    // Probably a static chunk.
                    continue;
                };
                let times = time_column.times_raw();
                if times.is_empty() {
                    continue;
                }

                cc.times_unique = times.windows(2).all(|w| w[0] < w[1]);

                // Find the position of `times[0]` in `unique_index_values`,
                // then verify that the chunk's rows align 1:1 with the next
                // `times.len()` entries. If they do, record the span; if any
                // hole exists (duplicate timestamps or another column's
                // chunks adding extra index values inside the range), leave
                // `dense_uiv_span = None`.
                let start = unique_index_values.partition_point(|t| t.as_i64() < times[0]);
                let span = Span::from_start_len(start, times.len());
                if unique_index_values.len() < span.end() {
                    continue;
                }
                let is_dense = std::iter::zip(&unique_index_values[span.range()], times)
                    .all(|(uiv, t)| uiv.as_i64() == *t);
                if is_dense {
                    cc.dense_uiv_span = Some(span);
                }
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_view_chunks(
        query: &QueryExpression,
        store: &ChunkStore,
        cache: &QueryCache,
        range_query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> (Option<usize>, Vec<Vec<ChunkBundle>>) {
        re_tracing::profile_function!();
        let mut view_pov_chunks_idx = query.filtered_is_not_null.as_ref().map(|_| usize::MAX);

        let view_chunks = view_contents
            .iter()
            .enumerate()
            .map(|(idx, selected_column)| match selected_column {
                ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => Vec::new(),

                ColumnDescriptor::Component(column) => {
                    let chunks = Self::fetch_chunks(
                        query,
                        store,
                        cache,
                        range_query,
                        &column.entity_path,
                        [column.component],
                    )
                    .unwrap_or_default();

                    if let Some(pov) = query.filtered_is_not_null.as_ref()
                        && column.matches(pov)
                    {
                        view_pov_chunks_idx = Some(idx);
                    }

                    chunks
                }
            })
            .collect();

        (view_pov_chunks_idx, view_chunks)
    }

    /// Returns all potentially relevant clear [`Chunk`]s for each unique entity path in the view contents.
    ///
    /// These chunks take recursive clear semantics into account and are guaranteed to be properly densified.
    /// The component data is stripped out, only the indices are left.
    fn fetch_clear_chunks(
        query: &QueryExpression,
        store: &ChunkStore,
        cache: &QueryCache,
        range_query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> IntMap<EntityPath, Vec<Chunk>> {
        re_tracing::profile_function!();

        /// Returns all the ancestors of an [`EntityPath`].
        ///
        /// Doesn't return `entity_path` itself.
        fn entity_path_ancestors(
            entity_path: &EntityPath,
        ) -> impl Iterator<Item = EntityPath> + use<> {
            std::iter::from_fn({
                let mut entity_path = entity_path.parent();
                move || {
                    let yielded = entity_path.clone()?;
                    entity_path = yielded.parent();
                    Some(yielded)
                }
            })
        }

        /// Given a [`Chunk`] containing a [`ClearIsRecursive`] column, returns a filtered version
        /// of that chunk where only rows with `ClearIsRecursive=true` are left.
        ///
        /// Returns `None` if the chunk either doesn't contain a `ClearIsRecursive` column or if
        /// the end result is an empty chunk.
        fn chunk_filter_recursive_only(chunk: &Chunk) -> Option<Chunk> {
            let list_array = chunk
                .components()
                .get_array(archetypes::Clear::descriptor_is_recursive().component)?;

            let values = list_array
                .values()
                .downcast_array_ref::<ArrowBooleanArray>()?;

            let indices = ArrowPrimitiveArray::from(
                values
                    .iter()
                    .enumerate()
                    .filter_map(|(index, is_recursive)| {
                        // can't fail - we're iterating over a 32-bit container
                        #[expect(clippy::cast_possible_wrap)]
                        (is_recursive == Some(true)).then_some(index as i32)
                    })
                    .collect_vec(),
            );

            let chunk = chunk.taken(&indices);

            (!chunk.is_empty()).then_some(chunk)
        }

        let components = [archetypes::Clear::descriptor_is_recursive().component];

        // All unique entity paths present in the view contents.
        let entity_paths: IntSet<EntityPath> = view_contents
            .iter()
            .filter_map(|col| col.entity_path().cloned())
            .collect();

        entity_paths
            .iter()
            .filter_map(|entity_path| {
                // For the entity itself, any chunk that contains clear data is relevant, recursive or not.
                // Just fetch everything we find.
                let flat_chunks =
                    Self::fetch_chunks(query, store, cache, range_query, entity_path, components)
                        .map(|chunks| chunks.into_iter().map(|cc| cc.chunk).collect_vec())
                        .unwrap_or_default();

                let recursive_chunks =
                    entity_path_ancestors(entity_path).flat_map(|ancestor_path| {
                        Self::fetch_chunks(
                            query,
                            store,
                            cache,
                            range_query,
                            &ancestor_path,
                            components,
                        )
                        .into_iter() // option
                        .flat_map(|chunks| chunks.into_iter().map(|cc| cc.chunk))
                        // NOTE: Ancestors' chunks are only relevant for the rows where `ClearIsRecursive=true`.
                        .filter_map(|chunk| chunk_filter_recursive_only(&chunk))
                    });

                // The component data is irrelevant.
                // We do not expose the actual tombstones to end-users, only their _effect_.
                let chunks = std::iter::chain(flat_chunks, recursive_chunks)
                    .map(|chunk| chunk.components_removed())
                    .collect_vec();

                (!chunks.is_empty()).then(|| (entity_path.clone(), chunks))
            })
            .collect()
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_chunks(
        query: &QueryExpression,
        _store: &ChunkStore,
        cache: &QueryCache,
        range_query: &RangeQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> Option<Vec<ChunkBundle>> {
        re_tracing::profile_function!();

        // NOTE: Keep in mind that the range APIs natively make sure that we will
        // either get a bunch of relevant _static_ chunks, or a bunch of relevant
        // _temporal_ chunks, but never both.
        //
        // TODO(cmc): Going through the cache is very useful in a Viewer context, but
        // not so much in an SDK context. Make it configurable.
        let results = cache.range(
            ChunkTrackingMode::Report,
            range_query,
            entity_path,
            components,
        );

        debug_assert!(
            results.components.len() <= 1,
            "cannot possibly get more than one component with this query"
        );

        results
            .components
            .into_iter()
            .next()
            .map(|(_component_descr, chunks)| {
                let filtered_index = query.filtered_index.as_ref();
                chunks
                    .into_iter()
                    .map(|chunk| {
                        // NOTE: Keep in mind that the range APIs would have already taken care
                        // of A) sorting the chunk on the `filtered_index` (and row-id) and
                        // B) densifying it according to the current `component_type`.
                        // Both of these are mandatory requirements for the deduplication logic to
                        // do what we want: keep the latest known value for `component_type` at all
                        // remaining unique index values all while taking row-id ordering semantics
                        // into account.
                        debug_assert!(
                            if let Some(index) = filtered_index {
                                chunk.is_timeline_sorted(index)
                            } else {
                                chunk.is_row_ids_sorted()
                            },
                            "the query cache should have already taken care of sorting (and densifying!) the chunk",
                        );

                        // TODO(cmc): That'd be more elegant, but right now there is no way to
                        // avoid allocations and copies when using Arrow's `ListArray`.
                        //
                        // let chunk = chunk.deduped_latest_on_index(&query.timeline);

                        ChunkBundle::new(chunk, filtered_index)
                    })
                    .collect_vec()
            })
    }

    /// The query used to instantiate this handle.
    #[inline]
    pub fn query(&self) -> &QueryExpression {
        &self.query
    }

    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression::view_contents`].
    #[inline]
    pub fn view_contents(&self) -> &ChunkColumnDescriptors {
        &self.init().view_contents
    }

    /// Describes the columns that make up this selection.
    ///
    /// The extra `usize` is the index in [`Self::view_contents`] that this selection points to.
    ///
    /// See [`QueryExpression::selection`].
    #[inline]
    pub fn selected_contents(&self) -> &[(usize, ColumnDescriptor)] {
        &self.init().selected_contents
    }

    /// All results returned by this handle will strictly follow this Arrow schema.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    #[inline]
    pub fn schema(&self) -> &ArrowSchemaRef {
        &self.init().arrow_schema
    }

    /// Advance all internal cursors so that the next row yielded will correspond to `row_idx`.
    ///
    /// Does nothing if `row_idx` is out of bounds.
    ///
    /// ## Performance
    ///
    /// This requires going through every chunk once, and for each chunk running a binary search if
    /// the chunk's time range contains the `index_value`.
    ///
    /// I.e.: it's pretty cheap already.
    #[inline]
    pub fn seek_to_row(&mut self, row_idx: usize) {
        let (state, iter_state) = self.init_iter();

        let Some(index_value) = state.unique_index_values.get(row_idx).copied() else {
            return;
        };

        iter_state.cur_row = row_idx as _;
        Self::seek_to_index_value_impl(state, iter_state, index_value);
    }

    /// Advance all internal cursors so that the next row yielded will correspond to `index_value`.
    ///
    /// If `index_value` isn't present in the dataset, this seeks to the first index value
    /// available past that point, if any.
    ///
    /// ## Performance
    ///
    /// This requires going through every chunk once, and for each chunk running a binary search if
    /// the chunk's time range contains the `index_value`.
    ///
    /// I.e.: it's pretty cheap already.
    #[tracing::instrument(level = "debug", skip_all)]
    fn seek_to_index_value_impl(
        state: &QueryHandleState,
        iter_state: &mut IterState,
        index_value: IndexValue,
    ) {
        re_tracing::profile_function!();

        if index_value.is_static() {
            for chunks in &mut iter_state.view_chunks {
                for cc in chunks {
                    cc.cursor = 0;
                    cc.exhausted = false;
                }
            }
            return;
        }

        for (state_chunks, iter_chunks) in
            std::iter::zip(&state.view_chunks, &mut iter_state.view_chunks)
        {
            for (bundle, cc) in std::iter::zip(state_chunks, iter_chunks) {
                // NOTE: The chunk has been densified already: its global time range is the same as
                // the time range for the specific component of interest.
                let Some(time_column) = bundle.chunk.timelines().get(&state.filtered_index) else {
                    continue;
                };

                let time_range = time_column.time_range();

                let new_cursor = if index_value < time_range.min() {
                    0
                } else if index_value > time_range.max() {
                    bundle.chunk.num_rows() as u64 /* yes, one past the end -- not a mistake */
                } else {
                    time_column
                        .times_raw()
                        .partition_point(|&time| time < index_value.as_i64())
                        as u64
                };

                cc.cursor = new_cursor;
                cc.exhausted = false;
            }
        }
    }

    /// How many rows of data will be returned?
    ///
    /// The number of rows depends and only depends on the _view contents_.
    /// The _selected contents_ has no influence on this value.
    pub fn num_rows(&self) -> u64 {
        self.init().unique_index_values.len() as _
    }

    /// Returns the row index of the last row whose index value is <= the given time,
    /// or `None` if no such row exists.
    pub fn row_index_at_or_before_time(&self, time: TimeInt) -> Option<u64> {
        let state = self.init();
        let idx = state.unique_index_values.partition_point(|t| *t <= time);
        if idx == 0 {
            None
        } else {
            Some((idx - 1) as u64)
        }
    }

    /// Returns the next row's worth of data.
    ///
    /// The returned vector of Arrow arrays strictly follows the schema specified by [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// Each cell in the result corresponds to the latest _locally_ known value at that particular point in
    /// the index, for each respective `ColumnDescriptor`.
    /// See [`QueryExpression::sparse_fill_strategy`] to go beyond local resolution.
    ///
    /// Example:
    /// ```ignore
    /// while let Some(row) = query_handle.next_row() {
    ///     // …
    /// }
    /// ```
    ///
    /// ## Pagination
    ///
    /// Use [`Self::seek_to_row`]:
    /// ```ignore
    /// query_handle.seek_to_row(42);
    /// for row in query_handle.into_iter().take(len) {
    ///     // …
    /// }
    /// ```
    #[inline]
    pub fn next_row(&mut self) -> Option<Vec<ArrayRef>> {
        // Trigger lazy state init through the immutable `&self` path before split-borrowing.
        let _ = self.init();
        let Self {
            engine,
            query,
            state,
            iter_state,
        } = self;
        let state = state.get().expect("state was just initialized by init()");
        let iter_state = iter_state.get_or_insert_with(|| IterState::new(state));
        engine.with(|store, cache| Self::_next_row(query, state, iter_state, store, cache))
    }

    /// Asynchronously returns the next row's worth of data.
    ///
    /// The returned vector of Arrow arrays strictly follows the schema specified by [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// Each cell in the result corresponds to the latest _locally_ known value at that particular point in
    /// the index, for each respective `ColumnDescriptor`.
    /// See [`QueryExpression::sparse_fill_strategy`] to go beyond local resolution.
    ///
    /// Example:
    /// ```ignore
    /// while let Some(row) = query_handle.next_row_async().await {
    ///     // …
    /// }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn next_row_async(
        &mut self,
    ) -> impl std::future::Future<Output = Option<Vec<ArrayRef>>> + use<E>
    where
        E: 'static + Send + Clone,
    {
        let Self {
            engine,
            query,
            state,
            iter_state,
        } = self;
        let res: Option<Option<_>> = engine.try_with(|store, cache| {
            let st = state.get_or_init(|| Self::init_(query, store, cache));
            let it = iter_state.get_or_insert_with(|| IterState::new(st));
            Self::_next_row(query, st, it, store, cache)
        });

        let engine = engine.clone();
        std::future::poll_fn(move |cx| {
            if let Some(row) = &res {
                std::task::Poll::Ready(row.clone())
            } else {
                // The lock is already held by a writer, we have to yield control back to the async
                // runtime, for now.
                // Before we do so, we need to schedule a callback that will be in charge of waking up
                // the async task once we can possibly make progress once again.

                // Commenting out this code should make the `async_barebones` test deadlock.
                rayon::spawn({
                    let engine = engine.clone();
                    let waker = cx.waker().clone();
                    move || {
                        engine.with(|_store, _cache| {
                            // This is of course optimistic -- we might end up right back here on
                            // next tick. That's fine.
                            waker.wake();
                        });
                    }
                });

                std::task::Poll::Pending
            }
        })
    }

    fn _next_row<'state>(
        query: &QueryExpression,
        state: &'state QueryHandleState,
        iter_state: &mut IterState,
        _store: &ChunkStore,
        cache: &QueryCache,
    ) -> Option<Vec<ArrowArrayRef>> {
        // re_tracing::profile_function!(); // too many and short-lived

        let mut scratch: Vec<Option<StreamingJoinState<'state>>> =
            Vec::with_capacity(state.view_chunks.len());
        let resolved = Self::_resolve_one_row(query, state, iter_state, cache, &mut scratch)?;

        // NOTE: Non-component entries have no data to slice, hence the optional layer.
        //
        // TODO(cmc): no point in slicing arrays that are not selected.
        let view_sliced_arrays: Vec<Option<_>> = scratch
            .iter()
            .enumerate()
            .map(|(view_idx, streaming_state)| {
                // NOTE: Reminder: the only reason the streaming state could be `None` here is
                // because this column does not have data for the current index value (i.e. `null`).
                let streaming_state = streaming_state.as_ref()?;
                let list_array = match streaming_state {
                    StreamingJoinState::StreamingJoinState(s) => {
                        debug_assert!(
                            s.chunk.components().iter().count() <= 1,
                            "cannot possibly get more than one component with this query"
                        );

                        s.chunk
                            .components()
                            .list_arrays()
                            .next()
                            .map(|list_array| list_array.slice(s.cursor as usize, 1))
                    }

                    StreamingJoinState::Retrofilled(unit) => {
                        let component = state
                            .view_contents
                            .get_index_or_component(view_idx)
                            .and_then(|col| {
                                if let ColumnDescriptor::Component(descr) = col {
                                    if let Some(component_type) = descr.component_type {
                                        component_type.sanity_check();
                                    }
                                    Some(descr.component)
                                } else {
                                    None
                                }
                            })?;
                        unit.components().get_array(component).cloned()
                    }
                };

                debug_assert!(
                    list_array.is_some(),
                    "This must exist or the chunk wouldn't have been sliced/retrofilled to start with."
                );

                // NOTE: This cannot possibly return None, see assert above.
                list_array
            })
            .collect();

        // TODO(cmc): It would likely be worth it to allocate all these possible
        // null-arrays ahead of time, and just return a pointer to those in the failure
        // case here.
        let selected_arrays = state
            .selected_contents
            .iter()
            .map(|(view_idx, column)| match column {
                ColumnDescriptor::RowId(_) => Option::zip(
                    state.view_chunks.first().and_then(|vec| vec.first()), // TODO(#9922): verify that using the row:ids from the first chunk always makes sense
                    iter_state.view_chunks.first().and_then(|vec| vec.first()),
                )
                .map(|(cc, cs)| as_array_ref(cc.chunk.row_ids_array().slice(cs.cursor as _, 1)))
                .unwrap_or_else(|| arrow::array::new_null_array(&RowId::arrow_datatype(), 1)),

                ColumnDescriptor::Time(descr) => resolved.get(descr.timeline().name()).map_or_else(
                    || arrow::array::new_null_array(&column.arrow_datatype(), 1),
                    |(_time, time_sliced)| {
                        descr.timeline().typ().make_arrow_array(time_sliced.clone())
                    },
                ),

                ColumnDescriptor::Component(_descr) => view_sliced_arrays
                    .get(*view_idx)
                    .cloned()
                    .flatten()
                    .map(into_arrow_ref)
                    .unwrap_or_else(|| arrow::array::new_null_array(&column.arrow_datatype(), 1)),
            })
            .collect_vec();

        debug_assert_eq!(state.arrow_schema.fields.len(), selected_arrays.len());

        Some(selected_arrays)
    }

    /// Resolve the streaming-join state for a single row.
    ///
    /// Returns `None` once the query is exhausted. On success, `view_streaming_state` holds the
    /// per-view-column resolved state for this row (component chunks, retrofills, statics) and
    /// the returned [`ResolvedRow`] holds the max value seen on each timeline.
    fn _resolve_one_row<'state>(
        query: &QueryExpression,
        state: &'state QueryHandleState,
        iter_state: &mut IterState,
        cache: &QueryCache,
        view_streaming_state: &mut Vec<Option<StreamingJoinState<'state>>>,
    ) -> Option<ResolvedRow> {
        let row_idx = iter_state.cur_row;
        iter_state.cur_row = row_idx + 1;
        let cur_index_value = state.unique_index_values.get(row_idx as usize)?;

        // First, we need to find, among all the chunks available for the current view contents,
        // what is their index value for the current row?
        //
        // NOTE: Non-component columns don't have a streaming state, hence the optional layer.
        view_streaming_state.clear();
        view_streaming_state.resize_with(state.view_chunks.len(), || None);
        let cur_index_value_i64 = cur_index_value.as_i64();
        for (view_column_idx, (view_chunks, iter_chunks)) in
            std::iter::zip(&state.view_chunks, &mut iter_state.view_chunks).enumerate()
        {
            let mut entry: Option<StreamingJoinStateEntry<'state>> = None;

            'overlaps: for (cc, cs) in std::iter::zip(view_chunks, iter_chunks) {
                // H1: skip chunks that already finished a prior row.
                if cs.exhausted {
                    continue 'overlaps;
                }

                // H3: chunks are sorted by `time_min` ascending — once we see a chunk
                // whose `time_min` is past `cur_index_value`, no later chunk can match
                // either.
                if cur_index_value_i64 < cc.time_min {
                    break 'overlaps;
                }

                // H2: chunks whose `time_max` is below `cur_index_value` are exhausted
                // for the rest of the iteration. Mark and skip.
                if cur_index_value_i64 > cc.time_max {
                    cs.exhausted = true;
                    continue 'overlaps;
                }

                // NOTE: Too soon to increment the cursor, we cannot know yet which chunks will or
                // will not be part of the current row.
                let mut cur_cursor_value = cs.cursor;

                let cur_index_times_empty: &[i64] = &[];
                let cur_index_times = cc
                    .chunk
                    .timelines()
                    .get(&state.filtered_index)
                    .map_or(cur_index_times_empty, |time_column| time_column.times_raw());
                let cur_index_row_ids = cc.chunk.row_ids_slice();

                let (index_value, cur_row_id) = 'walk: loop {
                    let (Some(mut index_value), Some(mut cur_row_id)) = (
                        cur_index_times
                            .get(cur_cursor_value as usize)
                            .copied()
                            .map(TimeInt::new_temporal),
                        cur_index_row_ids.get(cur_cursor_value as usize).copied(),
                    ) else {
                        // Cursor is past the last row of this chunk — exhausted forever.
                        cs.exhausted = true;
                        continue 'overlaps;
                    };

                    if index_value == *cur_index_value {
                        // TODO(cmc): Because of Arrow's `ListArray` limitations, we inline the
                        // "deduped_latest_on_index" logic here directly, which prevents a lot of
                        // unnecessary allocations and copies.
                        while let (Some(next_index_value), Some(next_row_id)) = (
                            cur_index_times
                                .get(cur_cursor_value as usize + 1)
                                .copied()
                                .map(TimeInt::new_temporal),
                            cur_index_row_ids
                                .get(cur_cursor_value as usize + 1)
                                .copied(),
                        ) {
                            if next_index_value == *cur_index_value {
                                index_value = next_index_value;
                                cur_row_id = next_row_id;
                                cur_cursor_value = cs.cursor + 1;
                                cs.cursor = cur_cursor_value;
                            } else {
                                break;
                            }
                        }

                        break 'walk (index_value, cur_row_id);
                    }

                    if index_value > *cur_index_value {
                        continue 'overlaps;
                    }

                    cur_cursor_value = cs.cursor + 1;
                    cs.cursor = cur_cursor_value;
                };

                debug_assert_eq!(index_value, *cur_index_value);

                if let Some(existing) = entry.as_mut() {
                    if cur_row_id > existing.row_id {
                        existing.chunk = &cc.chunk;
                        existing.cursor = cur_cursor_value;
                        existing.row_id = cur_row_id;
                    }
                } else {
                    entry = Some(StreamingJoinStateEntry {
                        chunk: &cc.chunk,
                        cursor: cur_cursor_value,
                        row_id: cur_row_id,
                    });
                }
            }

            view_streaming_state[view_column_idx] =
                entry.map(StreamingJoinState::StreamingJoinState);
        }

        // Static always wins, no matter what.
        for (selected_idx, static_state) in state.selected_static_values.iter().enumerate() {
            if let Some(unit) = static_state.clone() {
                let Some(view_idx) = state
                    .selected_contents
                    .get(selected_idx)
                    .map(|(view_idx, _)| *view_idx)
                else {
                    debug_panic!("selected_idx out of bounds");
                    continue;
                };

                let Some(streaming_state) = view_streaming_state.get_mut(view_idx) else {
                    debug_panic!("view_idx out of bounds");
                    continue;
                };

                *streaming_state = Some(StreamingJoinState::Retrofilled(unit));
            }
        }

        match query.sparse_fill_strategy {
            SparseFillStrategy::None => {}

            SparseFillStrategy::LatestAtGlobal => {
                // Everything that yielded `null` for the current iteration.
                let null_streaming_states = view_streaming_state
                    .iter_mut()
                    .enumerate()
                    .filter(|(_view_idx, streaming_state)| streaming_state.is_none());

                for (view_idx, streaming_state) in null_streaming_states {
                    let Some(ColumnDescriptor::Component(descr)) =
                        state.view_contents.get_index_or_component(view_idx)
                    else {
                        continue;
                    };

                    // NOTE: While it would be very tempting to resolve the latest-at state
                    // of the entire view contents at `filtered_index_range.start - 1` once
                    // during `QueryHandle` initialization, and then bootstrap off of that, that
                    // would effectively close the door to efficient pagination forever, since
                    // we'd have to iterate over all the pages to compute the right latest-at
                    // value at t+n (i.e. no more random access).
                    // Therefore, it is better to simply do this the "dumb" way.
                    //
                    // TODO(cmc): Still, as always, this can be made faster and smarter at
                    // the cost of some extra complexity (e.g. caching the result across
                    // consecutive nulls etc). Later.

                    let query =
                        re_chunk::LatestAtQuery::new(state.filtered_index, *cur_index_value);

                    let results = cache.latest_at(
                        ChunkTrackingMode::Report,
                        &query,
                        &descr.entity_path.clone(),
                        [descr.component],
                    );

                    *streaming_state = results
                        .components
                        .into_values()
                        .next()
                        .map(|unit| StreamingJoinState::Retrofilled(unit.clone()));
                }
            }
        }

        // We are stitching a bunch of unrelated cells together in order to create the final row
        // that is being returned.
        //
        // For this reason, we can only guarantee that the index being explicitly queried for
        // (`QueryExpression::filtered_index`) will match for all these cells.
        //
        // When it comes to other indices that the caller might have asked for, it is possible that
        // these different cells won't share the same values (e.g. two cells were found at
        // `log_time=100`, but one of them has `frame=3` and the other `frame=5`, for whatever
        // reason).
        // In order to deal with this, we keep track of the maximum value for every possible index
        // within the returned set of cells, and return that.
        //
        // TODO(cmc): In the future, it would be nice to make that either configurable, e.g.:
        // * return the minimum value instead of the max
        // * return the exact value for each component (that would be a _lot_ of columns!)
        // * etc
        let mut max_value_per_index: IntMap<TimelineName, (TimeInt, ArrowScalarBuffer<i64>)> =
            IntMap::default();
        view_streaming_state
            .iter()
            .flatten()
            .flat_map(|streaming_state| {
                match streaming_state {
                    StreamingJoinState::StreamingJoinState(s) => s.chunk.timelines(),
                    StreamingJoinState::Retrofilled(unit) => unit.timelines(),
                }
                .values()
                // NOTE: Cannot fail, just want to stay away from unwraps.
                .filter_map(move |time_column| {
                    let cursor = match streaming_state {
                        StreamingJoinState::StreamingJoinState(s) => s.cursor as usize,
                        StreamingJoinState::Retrofilled(_) => 0,
                    };
                    time_column
                        .times_raw()
                        .get(cursor)
                        .copied()
                        .map(TimeInt::new_temporal)
                        .map(|time| {
                            (
                                *time_column.timeline(),
                                (time, time_column.times_buffer().slice(cursor, 1)),
                            )
                        })
                })
            })
            .for_each(|(timeline, (time, time_sliced))| {
                max_value_per_index
                    .entry(*timeline.name())
                    .and_modify(|(max_time, max_time_sliced)| {
                        if time > *max_time {
                            *max_time = time;
                            *max_time_sliced = time_sliced.clone();
                        }
                    })
                    .or_insert((time, time_sliced));
            });

        if !cur_index_value.is_static() {
            // The current index value (if temporal) should be the one returned for the
            // queried index, no matter what.
            max_value_per_index.insert(
                state.filtered_index,
                (
                    *cur_index_value,
                    ArrowScalarBuffer::from(vec![cur_index_value.as_i64()]),
                ),
            );
        }

        Some(max_value_per_index)
    }

    /// Total number of rows emitted via the bulk fast path so far (see
    /// [`Self::try_bulk_emit_run`]). Exposed for tests / diagnostics.
    #[cfg(test)]
    pub(crate) fn bulk_emitted_rows(&self) -> u64 {
        self.iter_state.as_ref().map_or(0, |s| s.bulk_emitted_rows)
    }

    /// Append up to `max_rows` rows of data into freshly allocated per-column arrays.
    ///
    /// Throughput-oriented sibling of [`Self::next_row`]: shares the streaming-join machinery but
    /// amortizes per-row allocation by batching `MutableArrayData` extends and only finalizing
    /// to `ArrayRef` once per call.
    ///
    /// The returned [`NextNRowsOutput::columns`] strictly follows the schema specified by
    /// [`Self::schema`], with `num_rows == 0` signalling exhaustion.
    ///
    /// `max_bytes` caps the estimated output-buffer footprint of the batch (sum of
    /// per-row source-array bytes, computed from `ArrayData::get_array_memory_size /
    /// len`). Pass `usize::MAX` to disable the byte cap. The first row is always
    /// admitted regardless of the cap, so a single wide row can exceed `max_bytes`.
    #[inline]
    pub fn next_n_rows(&mut self, max_rows: usize, max_bytes: usize) -> NextNRowsOutput {
        re_tracing::profile_function!();
        let _ = self.init();
        let Self {
            engine,
            query,
            state,
            iter_state,
        } = self;
        let state = state.get().expect("state was just initialized by init()");
        let iter_state = iter_state.get_or_insert_with(|| IterState::new(state));
        engine.with(|store, cache| {
            Self::_next_n_rows(query, state, iter_state, store, cache, max_rows, max_bytes)
        })
    }

    /// Asynchronous sibling of [`Self::next_n_rows`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn next_n_rows_async(
        &mut self,
        max_rows: usize,
        max_bytes: usize,
    ) -> impl std::future::Future<Output = NextNRowsOutput> + use<'_, E>
    where
        E: 'static + Send + Clone,
    {
        let Self {
            engine,
            query,
            state,
            iter_state,
        } = self;

        // Retry on every poll: if `try_with` initially fails because a writer
        // holds the lock, the rayon-spawned `engine.with(..)` re-acquires the
        // lock and wakes us, but only a fresh `try_with` call here can actually
        // make progress. Capturing the result once at function entry would let
        // the future spin forever on a permanent `None`.
        //
        // State and iter-state are lazily initialized inside the `try_with` closure
        // so we never block on the engine lock — the future simply yields and is
        // re-polled when the writer releases the lock.
        std::future::poll_fn(move |cx| {
            let res = engine.try_with(|store, cache| {
                let st = state.get_or_init(|| Self::init_(query, store, cache));
                let it = iter_state.get_or_insert_with(|| IterState::new(st));
                Self::_next_n_rows(query, st, it, store, cache, max_rows, max_bytes)
            });

            if let Some(out) = res {
                std::task::Poll::Ready(out)
            } else {
                rayon::spawn({
                    let engine = engine.clone();
                    let waker = cx.waker().clone();
                    move || {
                        engine.with(|_store, _cache| {
                            waker.wake();
                        });
                    }
                });

                std::task::Poll::Pending
            }
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(max_rows, max_bytes))]
    fn _next_n_rows(
        query: &QueryExpression,
        state: &QueryHandleState,
        iter_state: &mut IterState,
        _store: &ChunkStore,
        cache: &QueryCache,
        max_rows: usize,
        max_bytes: usize,
    ) -> NextNRowsOutput {
        re_tracing::profile_function!();

        let n_selected = state.selected_contents.len();

        if max_rows == 0 {
            return NextNRowsOutput {
                columns: Vec::new(),
                num_rows: 0,
            };
        }

        // Clamp the up-front capacity hint: callers can legitimately pass a very
        // large `max_rows` (e.g. `usize::MAX` when only `max_bytes` is meant to
        // cap the batch) and `Vec::with_capacity(usize::MAX)` aborts with
        // "capacity overflow". 8192 covers every realistic batch size while
        // keeping the pre-allocation small; the vectors grow as needed.
        let cap_hint = max_rows.min(8192);

        let mut emitters: Vec<SelectedEmitter> = state
            .selected_contents
            .iter()
            .map(|(_view_idx, col)| match col {
                ColumnDescriptor::Component(_) | ColumnDescriptor::RowId(_) => {
                    SelectedEmitter::Source {
                        sources: Vec::new(),
                        source_ids: Vec::new(),
                        source_bytes_per_row: Vec::new(),
                        extends: Vec::with_capacity(cap_hint),
                    }
                }
                ColumnDescriptor::Time(_) => SelectedEmitter::Time {
                    values: Vec::with_capacity(cap_hint),
                    valid: Vec::with_capacity(cap_hint),
                },
            })
            .collect();

        let mut scratch: Vec<Option<StreamingJoinState<'_>>> =
            Vec::with_capacity(state.view_chunks.len());

        let mut num_rows = 0usize;
        let mut total_bytes = 0usize;
        loop {
            if num_rows >= max_rows {
                break;
            }
            // Always admit the first row so an empty batch is impossible while the
            // query has data (callers rely on `num_rows == 0` meaning exhausted).
            if num_rows > 0 && total_bytes >= max_bytes {
                break;
            }

            if let Some(emitted) = Self::try_bulk_emit_run(
                query,
                state,
                iter_state,
                &mut emitters,
                &mut total_bytes,
                max_rows,
                num_rows,
            ) {
                num_rows += emitted;
                iter_state.bulk_emitted_rows += emitted as u64;
                continue;
            }

            let Some(resolved) =
                Self::_resolve_one_row(query, state, iter_state, cache, &mut scratch)
            else {
                break;
            };

            for (selected_idx, (view_idx, column)) in state.selected_contents.iter().enumerate() {
                match column {
                    ColumnDescriptor::RowId(_) => {
                        let SelectedEmitter::Source {
                            sources,
                            source_ids,
                            source_bytes_per_row,
                            extends,
                        } = &mut emitters[selected_idx]
                        else {
                            debug_panic!("Source emitter expected for RowId column");
                            continue;
                        };

                        if let Some((cc, cs)) = Option::zip(
                            state.view_chunks.first().and_then(|v| v.first()),
                            iter_state.view_chunks.first().and_then(|v| v.first()),
                        ) {
                            // TODO(#9922): verify that using the row:ids from the first chunk
                            // always makes sense.
                            let id = std::ptr::from_ref::<Chunk>(&cc.chunk);
                            let pos = cs.cursor as usize;
                            let source_idx = SelectedEmitter::ensure_source(
                                sources,
                                source_ids,
                                source_bytes_per_row,
                                id,
                                || cc.chunk.row_ids_array().to_data(),
                            );
                            SelectedEmitter::push_run(
                                extends,
                                source_idx,
                                Span::from_start_len(pos, 1),
                            );
                            total_bytes =
                                total_bytes.saturating_add(source_bytes_per_row[source_idx]);
                        } else {
                            SelectedEmitter::push_nulls(extends, 1);
                        }
                    }

                    ColumnDescriptor::Time(descr) => {
                        let SelectedEmitter::Time { values, valid } = &mut emitters[selected_idx]
                        else {
                            debug_panic!("Time emitter expected for Time column");
                            continue;
                        };

                        if let Some((time, _)) = resolved.get(descr.timeline().name()) {
                            values.push(time.as_i64());
                            valid.push(true);
                        } else {
                            values.push(0);
                            valid.push(false);
                        }
                        total_bytes = total_bytes.saturating_add(std::mem::size_of::<i64>());
                    }

                    ColumnDescriptor::Component(_) => {
                        let SelectedEmitter::Source {
                            sources,
                            source_ids,
                            source_bytes_per_row,
                            extends,
                        } = &mut emitters[selected_idx]
                        else {
                            debug_panic!("Source emitter expected for Component column");
                            continue;
                        };

                        let streaming_state = scratch.get(*view_idx).and_then(|s| s.as_ref());
                        match streaming_state {
                            Some(StreamingJoinState::StreamingJoinState(s)) => {
                                let list_array_data = s
                                    .chunk
                                    .components()
                                    .list_arrays()
                                    .next()
                                    .map(|la| la.to_data());
                                if let Some(data) = list_array_data {
                                    let id = std::ptr::from_ref::<Chunk>(s.chunk);
                                    let source_idx = SelectedEmitter::ensure_source(
                                        sources,
                                        source_ids,
                                        source_bytes_per_row,
                                        id,
                                        || data,
                                    );
                                    SelectedEmitter::push_run(
                                        extends,
                                        source_idx,
                                        Span::from_start_len(s.cursor as usize, 1),
                                    );
                                    total_bytes = total_bytes
                                        .saturating_add(source_bytes_per_row[source_idx]);
                                } else {
                                    SelectedEmitter::push_nulls(extends, 1);
                                }
                            }
                            Some(StreamingJoinState::Retrofilled(unit)) => {
                                let component = state
                                    .view_contents
                                    .get_index_or_component(*view_idx)
                                    .and_then(|col| {
                                        if let ColumnDescriptor::Component(descr) = col {
                                            if let Some(component_type) = descr.component_type {
                                                component_type.sanity_check();
                                            }
                                            Some(descr.component)
                                        } else {
                                            None
                                        }
                                    });
                                let component_data = component.and_then(|c| {
                                    unit.components()
                                        .get_array(c)
                                        .cloned()
                                        .map(|arr| arr.to_data())
                                });
                                if let Some(data) = component_data {
                                    // UnitChunkShared derefs to Chunk; underlying address is
                                    // Arc-stable.
                                    let id = std::ptr::from_ref::<Chunk>(&**unit);
                                    let source_idx = SelectedEmitter::ensure_source(
                                        sources,
                                        source_ids,
                                        source_bytes_per_row,
                                        id,
                                        || data,
                                    );
                                    SelectedEmitter::push_run(
                                        extends,
                                        source_idx,
                                        Span::from_start_len(0, 1),
                                    );
                                    total_bytes = total_bytes
                                        .saturating_add(source_bytes_per_row[source_idx]);
                                } else {
                                    SelectedEmitter::push_nulls(extends, 1);
                                }
                            }
                            None => SelectedEmitter::push_nulls(extends, 1),
                        }
                    }
                }
            }

            num_rows += 1;
        }

        if num_rows == 0 {
            return NextNRowsOutput {
                columns: Vec::new(),
                num_rows: 0,
            };
        }

        re_tracing::profile_scope!("finalize");

        // Finalize each output column.
        let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(n_selected);
        for (selected_idx, emitter) in emitters.into_iter().enumerate() {
            let (_, column) = &state.selected_contents[selected_idx];
            let datatype = state.arrow_schema.field(selected_idx).data_type();
            match emitter {
                SelectedEmitter::Source {
                    sources,
                    extends,
                    source_ids: _,
                    source_bytes_per_row: _,
                } => {
                    if sources.is_empty() {
                        columns.push(arrow::array::new_null_array(datatype, num_rows));
                        continue;
                    }

                    if let [single] = extends.as_slice() {
                        // Fast-path: a single extend means no copying/concatenation is needed.
                        // We can either slice the source array directly or allocate a null array.
                        // This is commonly taken when `try_bulk_emit_run` succeeded.
                        re_tracing::profile_scope!("SelectedEmitter::Source fast-path");
                        match *single {
                            ColumnExtend::Range { source_idx, rows } => {
                                let sliced = sources[source_idx].slice(rows.start, rows.len);
                                columns.push(make_array(sliced));
                            }
                            ColumnExtend::Nulls { len } => {
                                columns.push(arrow::array::new_null_array(datatype, len));
                            }
                        }
                    } else {
                        re_tracing::profile_scope!("SelectedEmitter::Source slow-path");
                        let src_refs: Vec<&ArrayData> = sources.iter().collect();
                        let mut mutable = MutableArrayData::new(src_refs, true, num_rows);

                        for ext in &extends {
                            match ext {
                                ColumnExtend::Range { source_idx, rows } => {
                                    mutable.extend(*source_idx, rows.start, rows.end());
                                }
                                ColumnExtend::Nulls { len } => mutable.extend_nulls(*len),
                            }
                        }

                        columns.push(make_array(mutable.freeze()));
                    }
                }
                SelectedEmitter::Time { values, valid } => {
                    re_tracing::profile_scope!("SelectedEmitter::Time");

                    // The schema field's datatype is the source of truth. An
                    // `IndexColumnDescriptor::new_null` produces `datatype = Null`
                    // even though `descr.timeline().typ()` returns the placeholder
                    // `Sequence` (Int64); mirror `_next_row`, which falls back to
                    // `new_null_array(&column.arrow_datatype(), 1)` and therefore
                    // emits a `Null` array whenever the schema says so.
                    if matches!(datatype, ArrowDataType::Null) {
                        columns.push(arrow::array::new_null_array(datatype, num_rows));
                        continue;
                    }
                    let ColumnDescriptor::Time(descr) = column else {
                        debug_panic!("Time emitter on non-Time column");
                        columns.push(arrow::array::new_null_array(datatype, num_rows));
                        continue;
                    };
                    let nulls = if valid.iter().all(|v| *v) {
                        None
                    } else {
                        Some(valid.iter().copied().collect::<NullBuffer>())
                    };
                    columns.push(
                        descr
                            .timeline()
                            .typ()
                            .make_arrow_array_with_nulls(ArrowScalarBuffer::from(values), nulls),
                    );
                }
            }
        }

        debug_assert_eq!(columns.len(), state.arrow_schema.fields.len());

        NextNRowsOutput { columns, num_rows }
    }

    /// Fast path for [`Self::_next_n_rows`]: bulk-emit a run of rows from
    /// lonely+dense chunks without going through the per-row streaming join.
    ///
    /// For each non-empty view column at `cur_row`, classify the column's
    /// contribution to the upcoming run as either [`ColumnRunClass::Slice`]
    /// (`cur_row` sits inside a bulk-eligible chunk: `is_disjoint_in_column` +
    /// `times_unique` + `dense_uiv_span.is_some()`) or [`ColumnRunClass::Null`]
    /// (`cur_row` sits in a gap between chunks, or after every chunk).
    ///
    /// Any column that lands on a non-bulk-eligible chunk forces fall-through
    /// to the per-row path. Run length = `min` over per-column lengths,
    /// clamped to remaining `max_rows`. Gated by [`BULK_MIN_RUN`] to avoid
    /// bulk-machinery overhead on tiny runs, and by query-shape preconditions
    /// that would otherwise require per-row handling (sparse fill, pov filter,
    /// sampler).
    ///
    /// Returns the number of rows emitted, or `None` if the bulk path bailed
    /// (the caller must fall through to the per-row path).
    fn try_bulk_emit_run(
        query: &QueryExpression,
        state: &QueryHandleState,
        iter_state: &mut IterState,
        emitters: &mut [SelectedEmitter],
        total_bytes: &mut usize,
        max_rows: usize,
        num_rows: usize,
    ) -> Option<usize> {
        // No profiling scope until we've sure we're gonna do some actual work

        if query.sparse_fill_strategy != SparseFillStrategy::None {
            // Sparse fill performs `latest_at` lookups on null cells, mixing
            // chunk-derived cells with retrofilled `UnitChunkShared` data per
            // row. Bulk slicing cannot replicate that without per-row work.
            return None;
        }
        if query.filtered_is_not_null.is_some() {
            // The pov filter changes which rows enter `unique_index_values` and
            // can drop rows mid-chunk; the dense-with-uiv invariant no longer
            // holds, so bulk slicing would emit wrong rows.
            return None;
        }
        if query.using_index_values.is_some() {
            // `using_index_values` makes `unique_index_values` come from the
            // user, not the chunks. Chunks may have rows at index values that
            // are not requested (and vice versa), breaking dense-with-uiv.
            return None;
        }
        if query.filtered_index_values.is_some() {
            // `filtered_index_values` retains only the user-listed index
            // values, again breaking the dense-with-uiv invariant chunks were
            // classified against during `fill_bulk_metadata`.
            return None;
        }

        let remaining_max_rows = max_rows.saturating_sub(num_rows);
        if remaining_max_rows < BULK_MIN_RUN {
            // Run too short to amortize bulk-machinery overhead; let the
            // per-row path finish off the batch.
            return None;
        }

        let cur_row = iter_state.cur_row as usize;
        let uiv_total = state.unique_index_values.len();
        if uiv_total <= cur_row {
            // Query exhausted: no more rows to emit. The caller's outer loop
            // will see `_resolve_one_row` return `None` and break the batch.
            return None;
        }

        // re_tracing::profile_function!(); // even here we hit this too many times, with too much overhead

        // Used to test non-dense chunks for whether they overlap `cur_row`
        // along the timeline -- those chunks force the bulk path to bail.
        let cur_index_value_i64 = state.unique_index_values[cur_row].as_i64();

        // Per view-column classification. `None` for index columns
        // (RowId / Time) which carry no chunks of their own.
        let mut classes: Vec<Option<ColumnRunClass>> = Vec::with_capacity(state.view_chunks.len());
        let mut min_len = remaining_max_rows;
        let mut first_slice_view_idx: Option<usize> = None;
        let mut slice_count = 0usize;

        for (view_idx, chunks) in state.view_chunks.iter().enumerate() {
            if chunks.is_empty() {
                classes.push(None);
                continue;
            }

            let cs_chunks = &mut iter_state.view_chunks[view_idx];
            let mut found: Option<ColumnRunClass> = None;

            for (chunk_idx, (cc, cs)) in std::iter::zip(chunks, cs_chunks).enumerate() {
                if cs.exhausted {
                    continue;
                }
                let Some(span) = cc.dense_uiv_span else {
                    // Chunk lacks the `filtered_index` timeline, has duplicate
                    // timestamps, or has rows that don't line up 1:1 with
                    // `unique_index_values`. The bulk path can't slice it; if
                    // it overlaps `cur_row` (or starts after it, blocking the
                    // null-gap calculation), bail to per-row.
                    if cc.time_max < cur_index_value_i64 {
                        // Chunk strictly before cur_row -- mark exhausted so
                        // a subsequent per-row fallback doesn't re-examine it
                        // with a stale cursor.
                        cs.exhausted = true;
                        continue;
                    }
                    return None;
                };
                if span.end() <= cur_row {
                    // Chunk strictly before cur_row -- mark exhausted so a
                    // subsequent per-row fallback doesn't re-examine it with a
                    // stale cursor.
                    cs.exhausted = true;
                    continue;
                }
                if cur_row < span.start {
                    // Gap before this chunk -- column is null until it starts.
                    found = Some(ColumnRunClass::Null {
                        len: span.start - cur_row,
                    });
                    break;
                }
                // span.start <= cur_row < span.end: chunk covers cur_row.
                if !cc.is_disjoint_in_column || !cc.times_unique {
                    // Chunk overlaps another chunk in the column or has
                    // duplicate timestamps -- a bulk slice would miss the
                    // per-row dedup / max-rowid logic.
                    return None;
                }
                let cursor = cs.cursor as usize;
                if cursor != cur_row - span.start {
                    // The cursor was advanced by a prior per-row pass in a way
                    // that broke the dense+unique 1:1 row mapping (e.g. inline
                    // dedupe-forward skipped rows). A bulk slice from `cursor`
                    // would no longer line up with `cur_row`, so bail.
                    return None;
                }
                found = Some(ColumnRunClass::Slice {
                    chunk_idx,
                    rows: Span::from_start_len(cursor, span.end() - cur_row),
                });
                break;
            }

            let class = found.unwrap_or(ColumnRunClass::Null {
                len: uiv_total - cur_row,
            });
            let len = match class {
                ColumnRunClass::Slice { rows, .. } => {
                    if first_slice_view_idx.is_none() {
                        first_slice_view_idx = Some(view_idx);
                    }
                    slice_count += 1;
                    rows.len
                }
                ColumnRunClass::Null { len } => len,
            };
            min_len = min_len.min(len);
            classes.push(Some(class));
        }

        // Need at least one column with real data in this run: a run of
        // all-null rows has no chunk to draw RowId / other-timeline values
        // from, and emitting it via the bulk path would require special-casing
        // that the per-row path already handles.
        let rowid_view_idx = first_slice_view_idx?;

        if min_len < BULK_MIN_RUN {
            // After taking the `min` across all columns, the run is too short
            // to amortize bulk-machinery overhead; defer to the per-row path.
            return None;
        }

        // For other-timeline outputs the per-row path computes a max across
        // all contributing cells. Replicating that in bulk requires per-row
        // comparison against multiple slices; bail out conservatively when
        // both conditions hold simultaneously. Single-Slice runs (or runs
        // without other-timeline outputs) are exact.
        if 1 < slice_count {
            let has_other_timeline_selected = state.selected_contents.iter().any(|(_, col)| {
                if let ColumnDescriptor::Time(descr) = col {
                    *descr.timeline().name() != state.filtered_index
                } else {
                    false
                }
            });
            if has_other_timeline_selected {
                // `_resolve_one_row` computes max-across-cells for every
                // selected non-`filtered_index` timeline. With multiple Slice
                // columns we'd have to do that comparison per row inside the
                // bulk path; leave it to the per-row path until that's worth
                // implementing.
                return None;
            }
        }

        re_tracing::profile_scope!("bulk_emit", format!("len={min_len} slices={slice_count}"));

        // Source chunk used for RowId + non-filtered-index Time emission.
        let Some(ColumnRunClass::Slice {
            chunk_idx: rowid_chunk_idx,
            rows: rowid_rows,
        }) = classes[rowid_view_idx]
        else {
            unreachable!("rowid_view_idx came from a Slice classification")
        };
        let rowid_cursor = rowid_rows.start;
        let rowid_chunk = &state.view_chunks[rowid_view_idx][rowid_chunk_idx].chunk;

        // Emit `min_len` rows for every selected output column.
        for (selected_idx, (view_idx, column)) in state.selected_contents.iter().enumerate() {
            match column {
                ColumnDescriptor::RowId(_) => {
                    let SelectedEmitter::Source {
                        sources,
                        source_ids,
                        source_bytes_per_row,
                        extends,
                    } = &mut emitters[selected_idx]
                    else {
                        debug_panic!("Source emitter expected for RowId column");
                        continue;
                    };

                    let id = std::ptr::from_ref::<Chunk>(rowid_chunk);
                    let source_idx = SelectedEmitter::ensure_source(
                        sources,
                        source_ids,
                        source_bytes_per_row,
                        id,
                        || rowid_chunk.row_ids_array().to_data(),
                    );
                    SelectedEmitter::push_run(
                        extends,
                        source_idx,
                        Span::from_start_len(rowid_cursor, min_len),
                    );
                    *total_bytes = total_bytes
                        .saturating_add(source_bytes_per_row[source_idx].saturating_mul(min_len));
                }

                ColumnDescriptor::Time(descr) => {
                    let SelectedEmitter::Time { values, valid } = &mut emitters[selected_idx]
                    else {
                        debug_panic!("Time emitter expected for Time column");
                        continue;
                    };

                    if *descr.timeline().name() == state.filtered_index {
                        values.extend(
                            state.unique_index_values[cur_row..cur_row + min_len]
                                .iter()
                                .map(|t| t.as_i64()),
                        );
                        valid.extend(repeat_n(true, min_len));
                    } else if let Some(tc) = rowid_chunk.timelines().get(descr.timeline().name()) {
                        let times = tc.times_raw();
                        values.extend_from_slice(&times[rowid_cursor..rowid_cursor + min_len]);
                        valid.extend(repeat_n(true, min_len));
                    } else {
                        values.extend(repeat_n(0, min_len));
                        valid.extend(repeat_n(false, min_len));
                    }
                    *total_bytes = total_bytes
                        .saturating_add(std::mem::size_of::<i64>().saturating_mul(min_len));
                }

                ColumnDescriptor::Component(_) => {
                    let SelectedEmitter::Source {
                        sources,
                        source_ids,
                        source_bytes_per_row,
                        extends,
                    } = &mut emitters[selected_idx]
                    else {
                        debug_panic!("Source emitter expected for Component column");
                        continue;
                    };

                    match classes.get(*view_idx).copied().flatten() {
                        Some(ColumnRunClass::Slice { chunk_idx, rows }) => {
                            let cc = &state.view_chunks[*view_idx][chunk_idx];
                            let list_array_data = cc
                                .chunk
                                .components()
                                .list_arrays()
                                .next()
                                .map(|la| la.to_data());
                            if let Some(data) = list_array_data {
                                let id = std::ptr::from_ref::<Chunk>(&cc.chunk);
                                let source_idx = SelectedEmitter::ensure_source(
                                    sources,
                                    source_ids,
                                    source_bytes_per_row,
                                    id,
                                    || data,
                                );
                                SelectedEmitter::push_run(
                                    extends,
                                    source_idx,
                                    Span::from_start_len(rows.start, min_len),
                                );
                                *total_bytes = total_bytes.saturating_add(
                                    source_bytes_per_row[source_idx].saturating_mul(min_len),
                                );
                            } else {
                                SelectedEmitter::push_nulls(extends, min_len);
                            }
                        }
                        Some(ColumnRunClass::Null { .. }) | None => {
                            SelectedEmitter::push_nulls(extends, min_len);
                        }
                    }
                }
            }
        }

        // Advance per-column cursors for every Slice column. Null columns
        // have no cursor to advance -- their `cs.cursor` already points
        // past the gap (or is 0 with nothing scanned yet) and will be
        // reconsidered on the next iteration.
        for (view_idx, class) in classes.iter().enumerate() {
            if let Some(ColumnRunClass::Slice { chunk_idx, .. }) = class {
                let chunk_total = state.view_chunks[view_idx][*chunk_idx].chunk.num_rows();
                let cs = &mut iter_state.view_chunks[view_idx][*chunk_idx];
                cs.cursor += min_len as u64;
                if chunk_total <= cs.cursor as usize {
                    cs.exhausted = true;
                }
            }
        }
        iter_state.cur_row += min_len as u64;

        Some(min_len)
    }

    /// Calls [`Self::next_row`] and wraps the result in a [`ArrowRecordBatch`].
    ///
    /// Only use this if you absolutely need a [`ArrowRecordBatch`] as this adds a
    /// some overhead for schema validation.
    ///
    /// See [`Self::next_row`] for more information.
    #[inline]
    pub fn next_row_batch(&mut self) -> Option<ArrowRecordBatch> {
        let row = self.next_row()?;
        match ArrowRecordBatch::try_new_with_options(
            self.schema().clone(),
            row,
            // Explicitly setting row-count to one means it works even when there are no columns (e.g. due to heavy filtering)
            &RecordBatchOptions::new().with_row_count(Some(1)),
        ) {
            Ok(batch) => Some(batch),
            Err(err) => {
                if cfg!(debug_assertions) {
                    panic!("Failed to create record batch: {err}");
                } else {
                    re_log::error_once!("Failed to create record batch: {err}");
                    None
                }
            }
        }
    }

    #[inline]
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn next_row_batch_async(&mut self) -> Option<ArrowRecordBatch>
    where
        E: 'static + Send + Clone,
    {
        let row = self.next_row_async().await?;
        let row_count = row.first().map(|a| a.len()).unwrap_or(0);

        // If we managed to get a row, then the state must be initialized already.
        #[expect(clippy::unwrap_used)]
        let schema = self.state.get().unwrap().arrow_schema.clone();

        ArrowRecordBatch::try_new_with_options(
            schema,
            row,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
        .ok()
    }
}

impl<E: StorageEngineLike> QueryHandle<E> {
    /// Returns an iterator backed by [`Self::next_row`].
    pub fn iter(&mut self) -> impl Iterator<Item = Vec<ArrowArrayRef>> + '_ {
        std::iter::from_fn(move || self.next_row())
    }

    /// Returns an iterator backed by [`Self::next_row`].
    #[expect(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(mut self) -> impl Iterator<Item = Vec<ArrowArrayRef>> {
        std::iter::from_fn(move || self.next_row())
    }

    /// Returns an iterator backed by [`Self::next_row_batch`].
    pub fn batch_iter(&mut self) -> impl Iterator<Item = ArrowRecordBatch> + '_ {
        std::iter::from_fn(move || self.next_row_batch())
    }

    /// Returns an iterator backed by [`Self::next_row_batch`].
    pub fn into_batch_iter(mut self) -> impl Iterator<Item = ArrowRecordBatch> {
        std::iter::from_fn(move || self.next_row_batch())
    }
}

// ---

#[cfg(test)]
#[expect(clippy::iter_on_single_items)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{StringArray, UInt32Array};
    use arrow::compute::concat_batches;
    use insta::assert_snapshot;
    use re_arrow_util::format_record_batch;
    use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, TimePoint};
    use re_chunk_store::{
        AbsoluteTimeRange, ChunkStore, ChunkStoreConfig, ChunkStoreHandle, QueryExpression, TimeInt,
    };
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{EntityPath, Timeline, build_frame_nr, build_log_time};
    use re_query::StorageEngine;
    use re_sdk_types::{AnyValues, AsComponents as _, ComponentDescriptor};
    use re_sorbet::ComponentColumnSelector;
    use re_types_core::components;

    use super::*;
    use crate::{QueryCache, QueryEngine};

    /// Implement `Display` for `ArrowRecordBatch`
    struct DisplayRB(ArrowRecordBatch);

    impl std::fmt::Display for DisplayRB {
        #[inline]
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let width = 200;
            re_arrow_util::format_record_batch_with_width(&self.0, Some(width), f.sign_minus())
                .fmt(f)
        }
    }

    // NOTE: The best way to understand what these tests are doing is to run them in verbose mode,
    // e.g. `cargo t -p re_dataframe -- --show-output barebones`.
    // Each test will print the state of the store, the query being run, and the results that were
    // returned in the usual human-friendly format.
    // From there it is generally straightforward to infer what's going on.

    // TODO(cmc): at least one basic test for every feature in `QueryExpression`.
    // In no particular order:
    // * [x] filtered_index
    // * [x] filtered_index_range
    // * [x] filtered_index_values
    // * [x] view_contents
    // * [x] selection
    // * [x] filtered_is_not_null
    // * [x] sparse_fill_strategy
    // * [x] using_index_values
    //
    // In addition to those, some much needed extras:
    // * [x] num_rows
    // * [x] clears
    // * [ ] timelines returned with selection=none
    // * [x] pagination

    // TODO(cmc): At some point I'd like to stress multi-entity queries too, but that feels less
    // urgent considering how things are implemented (each entity lives in its own index, so it's
    // really just more of the same).

    /// All features disabled.
    #[test]
    fn barebones() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));

        // static
        {
            let query = QueryExpression::default();
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // temporal
        {
            let query = QueryExpression {
                filtered_index,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn sparse_fill_strategy_latestatglobal() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let query = QueryExpression {
            filtered_index,
            sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
            ..Default::default()
        };
        eprintln!("{query:#?}:");

        let mut query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let schema = query_handle.schema().clone();
        let batches = query_handle.batch_iter().collect_vec();
        let dataframe = concat_batches(&schema, &batches)?;
        eprintln!("{}", format_record_batch(&dataframe.clone()));

        assert_snapshot!(DisplayRB(dataframe));

        Ok(())
    }

    #[test]
    fn filtered_index_range() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let query = QueryExpression {
            filtered_index,
            filtered_index_range: Some(AbsoluteTimeRange::new(30, 60)),
            ..Default::default()
        };
        eprintln!("{query:#?}:");

        let mut query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let schema = query_handle.schema().clone();
        let batches = query_handle.batch_iter().collect_vec();
        let dataframe = concat_batches(&schema, &batches)?;
        eprintln!("{}", format_record_batch(&dataframe.clone()));

        assert_snapshot!(DisplayRB(dataframe));

        Ok(())
    }

    #[test]
    fn filtered_index_values() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let query = QueryExpression {
            filtered_index,
            filtered_index_values: Some(
                std::iter::chain(
                    [0, 30, 60, 90].into_iter().map(TimeInt::new_temporal),
                    std::iter::once(TimeInt::STATIC),
                )
                .collect(),
            ),
            ..Default::default()
        };
        eprintln!("{query:#?}:");

        let mut query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let schema = query_handle.schema().clone();
        let batches = query_handle.batch_iter().collect_vec();
        let dataframe = concat_batches(&schema, &batches)?;
        eprintln!("{}", format_record_batch(&dataframe.clone()));

        assert_snapshot!(DisplayRB(dataframe));

        Ok(())
    }

    #[test]
    fn using_index_values() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));

        // vanilla
        {
            let query = QueryExpression {
                filtered_index,
                using_index_values: Some(
                    std::iter::chain(
                        [0, 15, 30, 30, 45, 60, 75, 90]
                            .into_iter()
                            .map(TimeInt::new_temporal),
                        std::iter::once(TimeInt::STATIC),
                    )
                    .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // sparse-filled
        {
            let query = QueryExpression {
                filtered_index,
                using_index_values: Some(
                    std::iter::chain(
                        [0, 15, 30, 30, 45, 60, 75, 90]
                            .into_iter()
                            .map(TimeInt::new_temporal),
                        std::iter::once(TimeInt::STATIC),
                    )
                    .collect(),
                ),
                sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn filtered_is_not_null() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let entity_path: EntityPath = "this/that".into();

        // non-existing entity
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_points();

            let query = QueryExpression {
                filtered_index,
                filtered_is_not_null: Some(ComponentColumnSelector {
                    entity_path: "no/such/entity".into(),
                    component: component.to_string(),
                }),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // non-existing component
        {
            let query = QueryExpression {
                filtered_index,
                filtered_is_not_null: Some(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: "AFieldThatDoesntExist".to_owned(),
                }),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // MyPoint
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_points();

            let query = QueryExpression {
                filtered_index,
                filtered_is_not_null: Some(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: component.to_string(),
                }),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // MyColor
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_colors();

            let query = QueryExpression {
                filtered_index,
                filtered_is_not_null: Some(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: component.to_string(),
                }),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn view_contents() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let entity_path: EntityPath = "this/that".into();
        let filtered_index = Some(TimelineName::new("frame_nr"));

        // empty view
        {
            let query = QueryExpression {
                filtered_index,
                view_contents: Some(
                    [(entity_path.clone(), Some(Default::default()))]
                        .into_iter()
                        .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        {
            let query = QueryExpression {
                filtered_index,
                view_contents: Some(
                    [(
                        entity_path.clone(),
                        Some(
                            [
                                MyPoints::descriptor_labels().component,
                                MyPoints::descriptor_colors().component,
                                ComponentIdentifier::new("AColumnThatDoesntEvenExist"),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    )]
                    .into_iter()
                    .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn selection() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let entity_path: EntityPath = "this/that".into();
        let filtered_index = TimelineName::new("frame_nr");

        // empty selection
        {
            let query = QueryExpression {
                filtered_index: Some(filtered_index),
                selection: Some(vec![]),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // only indices (+ duplication)
        {
            let query = QueryExpression {
                filtered_index: Some(filtered_index),
                selection: Some(vec![
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from("ATimeColumnThatDoesntExist")),
                ]),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // duplication and non-existing
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_points();

            let query = QueryExpression {
                filtered_index: Some(filtered_index),
                selection: Some(vec![
                    // Duplication
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: component.to_string(),
                    }),
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: component.to_string(),
                    }),
                    // Non-existing entity
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: "non_existing_entity".into(),
                        component: component.to_string(),
                    }),
                    // Non-existing components
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: "MyPoints:AFieldThatDoesntExist".into(),
                    }),
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: "AFieldThatDoesntExist".into(),
                    }),
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: "AArchetypeNameThatDoesNotExist:positions".into(),
                    }),
                ]),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // static
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_labels();

            let query = QueryExpression {
                filtered_index: Some(filtered_index),
                selection: Some(vec![
                    // NOTE: This will force a crash if the selected indexes vs. view indexes are
                    // improperly handled.
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    //
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: component.to_string(),
                    }),
                ]),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn view_contents_and_selection() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let entity_path: EntityPath = "this/that".into();
        let filtered_index = TimelineName::new("frame_nr");

        // only components
        {
            let query = QueryExpression {
                filtered_index: Some(filtered_index),
                view_contents: Some(
                    [(
                        entity_path.clone(),
                        Some(
                            [
                                MyPoints::descriptor_colors().component,
                                MyPoints::descriptor_labels().component,
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    )]
                    .into_iter()
                    .collect(),
                ),
                selection: Some(vec![
                    ColumnSelector::Time(TimeColumnSelector::from(filtered_index)),
                    ColumnSelector::Time(TimeColumnSelector::from(*Timeline::log_time().name())),
                    ColumnSelector::Time(TimeColumnSelector::from(*Timeline::log_tick().name())),
                    //
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: MyPoints::descriptor_points().component.to_string(),
                    }),
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: MyPoints::descriptor_colors().component.to_string(),
                    }),
                    ColumnSelector::Component(ComponentColumnSelector {
                        entity_path: entity_path.clone(),
                        component: MyPoints::descriptor_labels().component.to_string(),
                    }),
                ]),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn clears() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        extend_nasty_store_with_clears(&mut store.write())?;
        eprintln!("{store}");

        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let entity_path = EntityPath::from("this/that");

        // barebones
        {
            let query = QueryExpression {
                filtered_index,
                view_contents: Some([(entity_path.clone(), None)].into_iter().collect()),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // sparse-filled
        {
            let query = QueryExpression {
                filtered_index,
                view_contents: Some([(entity_path.clone(), None)].into_iter().collect()),
                sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let schema = query_handle.schema().clone();
            let batches = query_handle.batch_iter().collect_vec();
            let dataframe = concat_batches(&schema, &batches)?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            // TODO(#7650): Those null values for `MyColor` on 10 and 20 look completely insane, but then again
            // static clear semantics in general are pretty unhinged right now, especially when
            // ranges are involved.

            assert_snapshot!(DisplayRB(dataframe));
        }

        Ok(())
    }

    #[test]
    fn pagination() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));
        let entity_path = EntityPath::from("this/that");

        // basic
        {
            let query = QueryExpression {
                filtered_index,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows(),
            );

            let expected_rows = query_handle.batch_iter().collect_vec();

            for _ in 0..3 {
                for i in 0..expected_rows.len() {
                    query_handle.seek_to_row(i);

                    let expected = concat_batches(
                        query_handle.schema(),
                        &expected_rows.iter().skip(i).take(3).cloned().collect_vec(),
                    )?;
                    let schema = query_handle.schema().clone();
                    let batches = query_handle.batch_iter().take(3).collect_vec();
                    let got = concat_batches(&schema, &batches)?;

                    let expected = format!("{:#?}", expected.columns());
                    let got = format!("{:#?}", got.columns());

                    similar_asserts::assert_eq!(expected, got);
                }
            }
        }

        // with pov
        {
            let ComponentDescriptor { component, .. } = MyPoints::descriptor_points();
            let query = QueryExpression {
                filtered_index,
                filtered_is_not_null: Some(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: component.to_string(),
                }),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows(),
            );

            let expected_rows = query_handle.batch_iter().collect_vec();

            for _ in 0..3 {
                for i in 0..expected_rows.len() {
                    query_handle.seek_to_row(i);

                    let expected = concat_batches(
                        query_handle.schema(),
                        &expected_rows.iter().skip(i).take(3).cloned().collect_vec(),
                    )?;
                    let schema = query_handle.schema().clone();
                    let batches = query_handle.batch_iter().take(3).collect_vec();
                    let got = concat_batches(&schema, &batches)?;

                    let expected = format!("{:#?}", expected.columns());
                    let got = format!("{:#?}", got.columns());

                    similar_asserts::assert_eq!(expected, got);
                }
            }
        }

        // with sampling
        {
            let query = QueryExpression {
                filtered_index,
                using_index_values: Some(
                    std::iter::chain(
                        [0, 15, 30, 30, 45, 60, 75, 90]
                            .into_iter()
                            .map(TimeInt::new_temporal),
                        std::iter::once(TimeInt::STATIC),
                    )
                    .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows(),
            );

            let expected_rows = query_handle.batch_iter().collect_vec();

            for _ in 0..3 {
                for i in 0..expected_rows.len() {
                    query_handle.seek_to_row(i);

                    let expected = concat_batches(
                        query_handle.schema(),
                        &expected_rows.iter().skip(i).take(3).cloned().collect_vec(),
                    )?;
                    let schema = query_handle.schema().clone();
                    let batches = query_handle.batch_iter().take(3).collect_vec();
                    let got = concat_batches(&schema, &batches)?;

                    let expected = format!("{:#?}", expected.columns());
                    let got = format!("{:#?}", got.columns());

                    similar_asserts::assert_eq!(expected, got);
                }
            }
        }

        // with sparse-fill
        {
            let query = QueryExpression {
                filtered_index,
                sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let mut query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows(),
            );

            let expected_rows = query_handle.batch_iter().collect_vec();

            for _ in 0..3 {
                for i in 0..expected_rows.len() {
                    query_handle.seek_to_row(i);

                    let expected = concat_batches(
                        query_handle.schema(),
                        &expected_rows.iter().skip(i).take(3).cloned().collect_vec(),
                    )?;
                    let schema = query_handle.schema().clone();
                    let batches = query_handle.batch_iter().take(3).collect_vec();
                    let got = concat_batches(&schema, &batches)?;

                    let expected = format!("{:#?}", expected.columns());
                    let got = format!("{:#?}", got.columns());

                    similar_asserts::assert_eq!(expected, got);
                }
            }
        }

        Ok(())
    }

    #[test]
    fn query_static_any_values() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStore::new_handle(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let any_values = AnyValues::default()
            .with_component_from_data("yak", Arc::new(StringArray::from(vec!["yuk"])))
            .with_component_from_data("foo", Arc::new(StringArray::from(vec!["bar"])))
            .with_component_from_data("baz", Arc::new(UInt32Array::from(vec![42u32])));

        let entity_path = EntityPath::from("test");

        let chunk0 = Chunk::builder(entity_path.clone())
            .with_serialized_batches(
                RowId::new(),
                TimePoint::default(),
                any_values.as_serialized_batches(),
            )
            .build()?;

        store.write().insert_chunk(&Arc::new(chunk0))?;

        let engine = QueryEngine::from_store(store);

        let query_expr = QueryExpression {
            view_contents: None,
            include_semantically_empty_columns: false,
            include_tombstone_columns: false,
            include_static_columns: re_chunk_store::StaticColumnSelection::Both,
            filtered_index: None,
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_is_not_null: None,
            sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
            selection: None,
        };

        let mut query_handle = engine.query(query_expr);

        let schema = query_handle.schema().clone();
        let batches = query_handle.batch_iter().collect_vec();
        let dataframe = concat_batches(&schema, &batches)?;
        eprintln!("{}", format_record_batch(&dataframe.clone()));

        assert_snapshot!(DisplayRB(dataframe));

        Ok(())
    }

    #[tokio::test]
    async fn async_barebones() -> anyhow::Result<()> {
        use tokio_stream::StreamExt as _;

        re_log::setup_logging();

        /// Wraps a [`QueryHandle`] in a [`Stream`].
        pub struct QueryHandleStream(pub QueryHandle<StorageEngine>);

        impl tokio_stream::Stream for QueryHandleStream {
            type Item = ArrowRecordBatch;

            #[inline]
            fn poll_next(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                let fut = self.0.next_row_batch_async();
                let fut = std::pin::pin!(fut);

                use std::future::Future as _;
                fut.poll(cx)
            }
        }

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        eprintln!("{store}");
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let engine_guard = query_engine.engine.write_arc();

        let filtered_index = Some(TimelineName::new("frame_nr"));

        // static
        let handle_static = tokio::spawn({
            let query_engine = query_engine.clone();
            async move {
                let query = QueryExpression::default();
                eprintln!("{query:#?}:");

                let query_handle = query_engine.query(query.clone());
                assert_eq!(
                    QueryHandleStream(query_engine.query(query.clone()))
                        .collect::<Vec<_>>()
                        .await
                        .len() as u64,
                    query_handle.num_rows()
                );
                let dataframe = concat_batches(
                    query_handle.schema(),
                    &QueryHandleStream(query_engine.query(query.clone()))
                        .collect::<Vec<_>>()
                        .await,
                )?;
                eprintln!("{}", format_record_batch(&dataframe.clone()));

                assert_snapshot!("async_barebones_static", DisplayRB(dataframe));

                Ok::<_, anyhow::Error>(())
            }
        });

        // temporal
        let handle_temporal = tokio::spawn({
            async move {
                let query = QueryExpression {
                    filtered_index,
                    ..Default::default()
                };
                eprintln!("{query:#?}:");

                let query_handle = query_engine.query(query.clone());
                assert_eq!(
                    QueryHandleStream(query_engine.query(query.clone()))
                        .collect::<Vec<_>>()
                        .await
                        .len() as u64,
                    query_handle.num_rows()
                );
                let dataframe = concat_batches(
                    query_handle.schema(),
                    &QueryHandleStream(query_engine.query(query.clone()))
                        .collect::<Vec<_>>()
                        .await,
                )?;
                eprintln!("{}", format_record_batch(&dataframe.clone()));

                assert_snapshot!("async_barebones_temporal", DisplayRB(dataframe));

                Ok::<_, anyhow::Error>(())
            }
        });

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let handle_queries = tokio::spawn(async move {
            let mut handle_static = std::pin::pin!(handle_static);
            let mut handle_temporal = std::pin::pin!(handle_temporal);

            // Poll the query handles, just once.
            //
            // Because the storage engine is already held by a writer, this will put them in a pending state,
            // waiting to be woken up. If nothing wakes them up, then this will simply deadlock.
            {
                // Although it might look scary, all we're doing is crafting a noop waker manually,
                // because `std::task::Waker::noop` is unstable.
                //
                // We'll use this to build a noop async context, so that we can poll our promises
                // manually.
                const RAW_WAKER_NOOP: std::task::RawWaker = {
                    const VTABLE: std::task::RawWakerVTable = std::task::RawWakerVTable::new(
                        |_| RAW_WAKER_NOOP, // Cloning just returns a new no-op raw waker
                        |_| {},             // `wake` does nothing
                        |_| {},             // `wake_by_ref` does nothing
                        |_| {},             // Dropping does nothing as we don't allocate anything
                    );
                    std::task::RawWaker::new(std::ptr::null(), &VTABLE)
                };

                #[expect(unsafe_code)]
                let mut cx = std::task::Context::from_waker(
                    // Safety: a Waker is just a privacy-preserving wrapper around a RawWaker.
                    unsafe {
                        &*std::ptr::from_ref::<std::task::RawWaker>(&RAW_WAKER_NOOP)
                            .cast::<std::task::Waker>()
                    },
                );

                use std::future::Future as _;
                assert!(handle_static.as_mut().poll(&mut cx).is_pending());
                assert!(handle_temporal.as_mut().poll(&mut cx).is_pending());
            }

            tx.send(()).unwrap();

            handle_static.await??;
            handle_temporal.await??;

            Ok::<_, anyhow::Error>(())
        });

        rx.await?;

        // Release the writer: the queries should now be able to stream to completion, provided
        // that _something_ wakes them up appropriately.
        drop(engine_guard);

        handle_queries.await??;

        Ok(())
    }

    /// Verifies that `next_n_rows` produces the same data as repeated `next_row` calls,
    /// across all sparse-fill strategies and a representative selection of queries.
    #[test]
    fn next_n_rows_matches_next_row() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = ChunkStoreHandle::new(create_nasty_store()?);
        let query_cache = QueryCache::new_handle(store.clone());
        let query_engine = QueryEngine::new(store.clone(), query_cache.clone());

        let filtered_index = Some(TimelineName::new("frame_nr"));

        // Cover: static, temporal, range-filtered, and LatestAtGlobal sparse fill.
        let queries = [
            QueryExpression::default(),
            QueryExpression {
                filtered_index,
                ..Default::default()
            },
            QueryExpression {
                filtered_index,
                filtered_index_range: Some(AbsoluteTimeRange::new(30, 60)),
                ..Default::default()
            },
            QueryExpression {
                filtered_index,
                sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
                ..Default::default()
            },
        ];

        for query in queries {
            // Reference path: row-by-row via existing `next_row`.
            let mut reference_handle = query_engine.query(query.clone());
            let reference_rows: Vec<_> = reference_handle.iter().collect();
            let total_rows = reference_rows.len();
            let n_fields = reference_handle.schema().fields.len();

            // Candidate path: many `next_n_rows` calls of size 64.
            let mut candidate_handle = query_engine.query(query.clone());
            let mut candidate_rows = 0usize;
            let mut candidate_columns: Vec<Vec<ArrowArrayRef>> =
                (0..n_fields).map(|_| Vec::new()).collect();
            loop {
                let out = candidate_handle.next_n_rows(64, usize::MAX);
                if out.num_rows == 0 {
                    break;
                }
                candidate_rows += out.num_rows;
                for (col_idx, arr) in out.columns.into_iter().enumerate() {
                    candidate_columns[col_idx].push(arr);
                }
            }

            assert_eq!(
                total_rows, candidate_rows,
                "row count mismatch for query {query:?}"
            );

            // Concatenate row-by-row reference into per-column arrays and compare.
            for (col_idx, candidate_parts) in candidate_columns.iter().enumerate() {
                let ref_parts: Vec<&dyn arrow::array::Array> = reference_rows
                    .iter()
                    .map(|row| row[col_idx].as_ref())
                    .collect();
                let cand_parts: Vec<&dyn arrow::array::Array> =
                    candidate_parts.iter().map(|a| a.as_ref()).collect();
                if ref_parts.is_empty() && cand_parts.is_empty() {
                    continue;
                }
                let reference =
                    re_arrow_util::concat_arrays(&ref_parts).expect("ref concat failed");
                let candidate =
                    re_arrow_util::concat_arrays(&cand_parts).expect("cand concat failed");
                assert_eq!(
                    reference.to_data(),
                    candidate.to_data(),
                    "column {col_idx} mismatch for query {query:?}",
                );
            }
        }

        Ok(())
    }

    /// Exercises the lonely-chunk bulk-emit fast path in `_next_n_rows`.
    ///
    /// Constructs a single-entity, single-component store with multiple disjoint
    /// chunks on the query timeline so that each chunk is `is_disjoint_in_column`,
    /// `times_unique`, and `is_dense_with_uiv`. Verifies that bulk emission
    /// produces the same dataframe as the per-row `next_row` reference path.
    #[test]
    fn next_n_rows_bulk_disjoint_single_column() -> anyhow::Result<()> {
        re_log::setup_logging();

        use re_log_types::TimeCell;
        use re_log_types::example_components::{MyPoint, MyPoints};

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );
        let entity_path = EntityPath::from("/disjoint");
        let timeline = TimelineName::new("frame");

        // Three pairwise-disjoint chunks, each dense (10 rows step 1, strictly
        // increasing) so all three should satisfy `is_bulk_eligible`.
        let chunk_ranges: [(i64, i64); 3] = [(10, 19), (30, 39), (50, 59)];
        for (lo, hi) in chunk_ranges {
            let mut builder = Chunk::builder(entity_path.clone());
            for t in lo..=hi {
                #[expect(clippy::cast_sign_loss)]
                let pt = MyPoint::from_iter((t as u32)..(t as u32 + 1));
                builder = builder.with_archetype(
                    RowId::new(),
                    [(timeline, TimeCell::from_sequence(t))],
                    &MyPoints::new(pt),
                );
            }
            store.insert_chunk(&std::sync::Arc::new(builder.build()?))?;
        }

        let store_handle = ChunkStoreHandle::new(store);
        let query_cache = QueryCache::new_handle(store_handle.clone());
        let query_engine = QueryEngine::new(store_handle, query_cache);

        let query = QueryExpression {
            filtered_index: Some(timeline),
            view_contents: Some(
                [(
                    entity_path.clone(),
                    Some(
                        [MyPoints::descriptor_points().component]
                            .into_iter()
                            .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        };

        let reference: Vec<_> = query_engine.query(query.clone()).iter().collect();
        let total_rows = reference.len();
        assert_eq!(total_rows, 30, "3 chunks x 10 rows each");

        let mut candidate_handle = query_engine.query(query);
        let n_fields = candidate_handle.schema().fields.len();
        let mut candidate_columns: Vec<Vec<ArrowArrayRef>> =
            (0..n_fields).map(|_| Vec::new()).collect();
        let mut candidate_rows = 0usize;
        loop {
            let out = candidate_handle.next_n_rows(64, usize::MAX);
            if out.num_rows == 0 {
                break;
            }
            candidate_rows += out.num_rows;
            for (col_idx, arr) in out.columns.into_iter().enumerate() {
                candidate_columns[col_idx].push(arr);
            }
        }
        assert_eq!(candidate_rows, total_rows);

        // Guard against regressions that silently disable the bulk fast path.
        assert!(
            candidate_handle.bulk_emitted_rows() > 0,
            "bulk fast path was never taken; this test is no longer exercising it",
        );

        for (col_idx, parts) in candidate_columns.iter().enumerate() {
            let ref_parts: Vec<&dyn arrow::array::Array> =
                reference.iter().map(|row| row[col_idx].as_ref()).collect();
            let cand_parts: Vec<&dyn arrow::array::Array> =
                parts.iter().map(|a| a.as_ref()).collect();
            let r = re_arrow_util::concat_arrays(&ref_parts).expect("ref concat");
            let c = re_arrow_util::concat_arrays(&cand_parts).expect("cand concat");
            assert_eq!(r.to_data(), c.to_data(), "column {col_idx} mismatch");
        }

        Ok(())
    }

    /// Multi-column variant of [`Self::next_n_rows_bulk_disjoint_single_column`].
    ///
    /// Inserts disjoint chunks each carrying *three* components on the same
    /// entity. After per-component densification, each view column should still
    /// be bulk-eligible (same time grid, no overlaps), so the multi-column bulk
    /// path is exercised.
    #[test]
    fn next_n_rows_bulk_disjoint_multi_column() -> anyhow::Result<()> {
        re_log::setup_logging();

        use re_log_types::TimeCell;
        use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );
        let entity_path = EntityPath::from("/multi");
        let timeline = TimelineName::new("frame");

        let chunk_ranges: [(i64, i64); 3] = [(10, 19), (30, 39), (50, 59)];
        for (lo, hi) in chunk_ranges {
            let mut builder = Chunk::builder(entity_path.clone());
            for t in lo..=hi {
                #[expect(clippy::cast_sign_loss)]
                let n = t as u32;
                let archetype = MyPoints::new(MyPoint::from_iter(n..n + 1))
                    .with_colors([MyColor::from(0xFF000000_u32 | n)])
                    .with_labels([MyLabel(format!("L{n}"))]);
                builder = builder.with_archetype(
                    RowId::new(),
                    [(timeline, TimeCell::from_sequence(t))],
                    &archetype,
                );
            }
            store.insert_chunk(&std::sync::Arc::new(builder.build()?))?;
        }

        let store_handle = ChunkStoreHandle::new(store);
        let query_cache = QueryCache::new_handle(store_handle.clone());
        let query_engine = QueryEngine::new(store_handle, query_cache);

        let query = QueryExpression {
            filtered_index: Some(timeline),
            view_contents: Some(
                [(
                    entity_path.clone(),
                    Some(
                        [
                            MyPoints::descriptor_points().component,
                            MyPoints::descriptor_colors().component,
                            MyPoints::descriptor_labels().component,
                        ]
                        .into_iter()
                        .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        };

        let reference: Vec<_> = query_engine.query(query.clone()).iter().collect();
        let total_rows = reference.len();
        assert_eq!(total_rows, 30, "3 chunks x 10 rows each");

        let mut candidate_handle = query_engine.query(query);
        let n_fields = candidate_handle.schema().fields.len();
        let mut candidate_columns: Vec<Vec<ArrowArrayRef>> =
            (0..n_fields).map(|_| Vec::new()).collect();
        let mut candidate_rows = 0usize;
        loop {
            let out = candidate_handle.next_n_rows(64, usize::MAX);
            if out.num_rows == 0 {
                break;
            }
            candidate_rows += out.num_rows;
            for (col_idx, arr) in out.columns.into_iter().enumerate() {
                candidate_columns[col_idx].push(arr);
            }
        }
        assert_eq!(candidate_rows, total_rows);

        // Guard against regressions that silently disable the bulk fast path.
        assert!(
            candidate_handle.bulk_emitted_rows() > 0,
            "bulk fast path was never taken; this test is no longer exercising it",
        );

        for (col_idx, parts) in candidate_columns.iter().enumerate() {
            let ref_parts: Vec<&dyn arrow::array::Array> =
                reference.iter().map(|row| row[col_idx].as_ref()).collect();
            let cand_parts: Vec<&dyn arrow::array::Array> =
                parts.iter().map(|a| a.as_ref()).collect();
            let r = re_arrow_util::concat_arrays(&ref_parts).expect("ref concat");
            let c = re_arrow_util::concat_arrays(&cand_parts).expect("cand concat");
            assert_eq!(r.to_data(), c.to_data(), "column {col_idx} mismatch");
        }

        Ok(())
    }

    /// Returns a very nasty [`ChunkStore`] with all kinds of partial updates, chunk overlaps,
    /// repeated timestamps, duplicated chunks, partial multi-timelines, flat and recursive clears, etc.
    fn create_nasty_store() -> anyhow::Result<ChunkStore> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let entity_path = EntityPath::from("/this/that");

        let frame1 = TimeInt::new_temporal(10);
        let frame2 = TimeInt::new_temporal(20);
        let frame3 = TimeInt::new_temporal(30);
        let frame4 = TimeInt::new_temporal(40);
        let frame5 = TimeInt::new_temporal(50);
        let frame6 = TimeInt::new_temporal(60);
        let frame7 = TimeInt::new_temporal(70);

        let points1 = MyPoint::from_iter(0..1);
        let points2 = MyPoint::from_iter(1..2);
        let points3 = MyPoint::from_iter(2..3);
        let points4 = MyPoint::from_iter(3..4);
        let points5 = MyPoint::from_iter(4..5);
        let points6 = MyPoint::from_iter(5..6);
        let points7_1 = MyPoint::from_iter(6..7);
        let points7_2 = MyPoint::from_iter(7..8);
        let points7_3 = MyPoint::from_iter(8..9);

        let colors3 = MyColor::from_iter(2..3);
        let colors4 = MyColor::from_iter(3..4);
        let colors5 = MyColor::from_iter(4..5);
        let colors7 = MyColor::from_iter(6..7);

        let labels1 = vec![MyLabel("a".to_owned())];
        let labels2 = vec![MyLabel("b".to_owned())];
        let labels3 = vec![MyLabel("c".to_owned())];

        let row_id1_1 = RowId::new();
        let row_id1_3 = RowId::new();
        let row_id1_5 = RowId::new();
        let row_id1_7_1 = RowId::new();
        let row_id1_7_2 = RowId::new();
        let row_id1_7_3 = RowId::new();
        let chunk1_1 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id1_1,
                [build_frame_nr(frame1), build_log_time(frame1.into())],
                [
                    (MyPoints::descriptor_points(), Some(&points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(&labels1 as _)), // shadowed by static
                ],
            )
            .with_sparse_component_batches(
                row_id1_3,
                [build_frame_nr(frame3), build_log_time(frame3.into())],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id1_5,
                [build_frame_nr(frame5), build_log_time(frame5.into())],
                [
                    (MyPoints::descriptor_points(), Some(&points5 as _)),
                    (MyPoints::descriptor_colors(), None),
                ],
            )
            .with_sparse_component_batches(
                row_id1_7_1,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoints::descriptor_points(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_2,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoints::descriptor_points(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_3,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoints::descriptor_points(), Some(&points7_3 as _))],
            )
            .build()?;

        let chunk1_1 = Arc::new(chunk1_1);
        store.insert_chunk(&chunk1_1)?;
        let chunk1_2 = Arc::new(chunk1_1.clone_as(ChunkId::new(), RowId::new()));
        store.insert_chunk(&chunk1_2)?; // x2 !
        let chunk1_3 = Arc::new(chunk1_1.clone_as(ChunkId::new(), RowId::new()));
        store.insert_chunk(&chunk1_3)?; // x3 !!

        let row_id2_2 = RowId::new();
        let row_id2_3 = RowId::new();
        let row_id2_4 = RowId::new();
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id2_2,
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                row_id2_3,
                [build_frame_nr(frame3)],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2_4,
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .build()?;

        let chunk2 = Arc::new(chunk2);
        store.insert_chunk(&chunk2)?;

        let row_id3_2 = RowId::new();
        let row_id3_4 = RowId::new();
        let row_id3_6 = RowId::new();
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id3_2,
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                row_id3_4,
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .with_sparse_component_batches(
                row_id3_6,
                [build_frame_nr(frame6)],
                [(MyPoints::descriptor_points(), Some(&points6 as _))],
            )
            .build()?;

        let chunk3 = Arc::new(chunk3);
        store.insert_chunk(&chunk3)?;

        let row_id4_4 = RowId::new();
        let row_id4_5 = RowId::new();
        let row_id4_7 = RowId::new();
        let chunk4 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id4_4,
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_colors(), Some(&colors4 as _))],
            )
            .with_sparse_component_batches(
                row_id4_5,
                [build_frame_nr(frame5)],
                [(MyPoints::descriptor_colors(), Some(&colors5 as _))],
            )
            .with_sparse_component_batches(
                row_id4_7,
                [build_frame_nr(frame7)],
                [(MyPoints::descriptor_colors(), Some(&colors7 as _))],
            )
            .build()?;

        let chunk4 = Arc::new(chunk4);
        store.insert_chunk(&chunk4)?;

        let row_id5_1 = RowId::new();
        let chunk5 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id5_1,
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels2 as _))],
            )
            .build()?;

        let chunk5 = Arc::new(chunk5);
        store.insert_chunk(&chunk5)?;

        let row_id6_1 = RowId::new();
        let chunk6 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id6_1,
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels3 as _))],
            )
            .build()?;

        let chunk6 = Arc::new(chunk6);
        store.insert_chunk(&chunk6)?;

        Ok(store)
    }

    fn extend_nasty_store_with_clears(store: &mut ChunkStore) -> anyhow::Result<()> {
        let entity_path = EntityPath::from("/this/that");
        let entity_path_parent = EntityPath::from("/this");
        let entity_path_root = EntityPath::from("/");

        let frame35 = TimeInt::new_temporal(35);
        let frame55 = TimeInt::new_temporal(55);
        let frame60 = TimeInt::new_temporal(60);
        let frame65 = TimeInt::new_temporal(65);

        let clear_flat = components::ClearIsRecursive(false.into());
        let clear_recursive = components::ClearIsRecursive(true.into());

        let row_id1_1 = RowId::new();
        let chunk1 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id1_1,
                TimePoint::default(),
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_flat as _),
                )],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        store.insert_chunk(&chunk1)?;

        // NOTE: This tombstone will never have any visible effect.
        //
        // Tombstones still obey the same rules as other all other data, specifically: if a component
        // has been statically logged for an entity, it shadows any temporal data for that same
        // component on that same entity.
        //
        // In this specific case, `this/that` already has been logged a static clear, so further temporal
        // clears will be ignored.
        //
        // It's pretty weird, but then again static clear semantics in general are very weird.
        let row_id2_1 = RowId::new();
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id2_1,
                [build_frame_nr(frame35), build_log_time(frame35.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_recursive as _),
                )],
            )
            .build()?;

        let chunk2 = Arc::new(chunk2);
        store.insert_chunk(&chunk2)?;

        let row_id3_1 = RowId::new();
        let chunk3 = Chunk::builder(entity_path_root.clone())
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame55), build_log_time(frame55.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_flat as _),
                )],
            )
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame60), build_log_time(frame60.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_recursive as _),
                )],
            )
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame65), build_log_time(frame65.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_flat as _),
                )],
            )
            .build()?;

        let chunk3 = Arc::new(chunk3);
        store.insert_chunk(&chunk3)?;

        let row_id4_1 = RowId::new();
        let chunk4 = Chunk::builder(entity_path_parent.clone())
            .with_sparse_component_batches(
                row_id4_1,
                [build_frame_nr(frame60), build_log_time(frame60.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_flat as _),
                )],
            )
            .build()?;

        let chunk4 = Arc::new(chunk4);
        store.insert_chunk(&chunk4)?;

        let row_id5_1 = RowId::new();
        let chunk5 = Chunk::builder(entity_path_parent.clone())
            .with_sparse_component_batches(
                row_id5_1,
                [build_frame_nr(frame65), build_log_time(frame65.into())],
                [(
                    archetypes::Clear::descriptor_is_recursive(),
                    Some(&clear_recursive as _),
                )],
            )
            .build()?;

        let chunk5 = Arc::new(chunk5);
        store.insert_chunk(&chunk5)?;

        Ok(())
    }

    /// Build a store containing `n_chunks` disjoint single-component chunks, each holding
    /// `rows_per_chunk` rows. Chunk `k` covers the time range
    /// `[k * rows_per_chunk, (k+1) * rows_per_chunk)`. Chunks are inserted in `order`.
    fn build_disjoint_chunk_store(
        n_chunks: usize,
        rows_per_chunk: usize,
        order: &[usize],
    ) -> anyhow::Result<ChunkStore> {
        assert_eq!(order.len(), n_chunks);
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let entity_path = EntityPath::from("/disjoint");

        let mut chunks: Vec<Arc<Chunk>> = Vec::with_capacity(n_chunks);
        for chunk_idx in 0..n_chunks {
            let mut builder = Chunk::builder(entity_path.clone());
            for local_row in 0..rows_per_chunk {
                let global_row = chunk_idx * rows_per_chunk + local_row;
                #[expect(clippy::cast_possible_wrap)]
                let frame = TimeInt::new_temporal(global_row as i64);
                let points = MyPoint::from_iter(global_row as u32..global_row as u32 + 1);
                builder = builder.with_sparse_component_batches(
                    RowId::new(),
                    [build_frame_nr(frame)],
                    [(MyPoints::descriptor_points(), Some(&points as _))],
                );
            }
            chunks.push(Arc::new(builder.build()?));
        }

        for &idx in order {
            store.insert_chunk(&chunks[idx])?;
        }

        Ok(store)
    }

    fn run_query_collect_rows(
        store: &ChunkStoreHandle,
        query: QueryExpression,
    ) -> anyhow::Result<ArrowRecordBatch> {
        let cache = QueryCache::new_handle(store.clone());
        let engine = QueryEngine::new(store.clone(), cache);
        let mut handle = engine.query(query);
        let schema = handle.schema().clone();
        let batches = handle.batch_iter().collect_vec();
        Ok(concat_batches(&schema, &batches)?)
    }

    /// Build `n_chunks` single-component chunks on the same entity whose time
    /// ranges overlap with their neighbors.
    ///
    /// Chunk `k` covers `frame_nr ∈ [k * step, k * step + chunk_width)`. With
    /// `step < chunk_width`, neighboring chunks share `chunk_width - step` frames.
    /// Each chunk uses a distinct payload offset so the streaming-join's
    /// max-`RowId` overlap resolution at `query.rs:1243` has a meaningful winner
    /// to pick.
    fn build_overlapping_chunks(
        n_chunks: usize,
        chunk_width: u32,
        step: u32,
    ) -> anyhow::Result<Vec<Arc<Chunk>>> {
        assert!(n_chunks >= 1);
        assert!(step < chunk_width, "chunks must actually overlap");

        let entity_path = EntityPath::from("/overlap");
        let mut chunks: Vec<Arc<Chunk>> = Vec::with_capacity(n_chunks);
        for k in 0..n_chunks {
            let mut builder = Chunk::builder(entity_path.clone());
            #[expect(clippy::cast_possible_truncation)]
            let base = (k as u32) * step;
            // Distinct payload so we can detect which chunk's data ended up in the row.
            #[expect(clippy::cast_possible_truncation)]
            let payload_offset = (k as u32) * 1_000;
            for f in base..base + chunk_width {
                let points = MyPoint::from_iter((f + payload_offset)..(f + payload_offset + 1));
                let frame = TimeInt::new_temporal(i64::from(f));
                builder = builder.with_sparse_component_batches(
                    RowId::new(),
                    [build_frame_nr(frame)],
                    [(MyPoints::descriptor_points(), Some(&points as _))],
                );
            }
            chunks.push(Arc::new(builder.build()?));
        }
        Ok(chunks)
    }

    fn build_overlapping_chunk_store(
        chunks: &[Arc<Chunk>],
        order: &[usize],
    ) -> anyhow::Result<ChunkStore> {
        assert_eq!(order.len(), chunks.len());
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );
        for &idx in order {
            store.insert_chunk(&chunks[idx])?;
        }
        Ok(store)
    }

    /// Verifies that the H3 init-sort by `time_min` (which is now unconditional, vs. the
    /// previous "only after clear-chunk merge" policy) leaves emitted output unchanged
    /// when two chunks overlap on the same `filtered_index` value, exercised through both
    /// the batched (`next_n_rows` -> `next_row_batch`) and the per-row (`next_row` ->
    /// `_resolve_one_row`) paths.
    ///
    /// Why both paths matter: the streaming-join's overlap resolution at `query.rs:1243`
    /// picks max-`RowId` for value columns, which is order-independent by construction.
    /// The `RowId` column itself is sourced from `view_chunks.first()`
    /// (`query.rs:1101-1107`, `1635-1656`). Once `view_chunks` is sorted by `time_min`,
    /// the "first chunk" identity becomes a function of `time_min` rather than insert
    /// order — so insert-order invariance is the correct assertion either way.
    ///
    /// Note: the `RowId` column itself is intentionally not in the selection. Today,
    /// `indices_and_components()` (per its `TODO(#9922)` doc) excludes `RowId`, so the
    /// first view is always a `Time` column with empty `view_chunks[0]`, which makes the
    /// `view_chunks.first().and_then(|vec| vec.first())` lookup return `None` and emit
    /// a null array — incompatible with the non-nullable `RowId` schema field. When
    /// `#9922` is fixed and `RowId` emission picks a real chunk, the H3 sort makes that
    /// choice a pure function of `time_min`, so this test's insert-order-invariance
    /// assertion will continue to hold over the full record batch.
    #[test]
    fn pruning_walk_overlapping_chunks_rowid_invariance() -> anyhow::Result<()> {
        re_log::setup_logging();

        // 3 chunks: A=[0,5), B=[3,8), C=[6,11). Pairwise overlaps on frames
        // {3,4} (A∩B) and {6,7} (B∩C). Distinct payload offsets per chunk so
        // the max-RowId overlap winner at `query.rs:1243` has a real choice
        // to make.
        const N_CHUNKS: usize = 3;
        const CHUNK_WIDTH: u32 = 5;
        const STEP: u32 = 3;
        let chunks = build_overlapping_chunks(N_CHUNKS, CHUNK_WIDTH, STEP)?;

        let forward: Vec<usize> = (0..N_CHUNKS).collect();
        let reversed: Vec<usize> = (0..N_CHUNKS).rev().collect();
        let shuffled: Vec<usize> = vec![1, 2, 0];

        let store_fwd = ChunkStoreHandle::new(build_overlapping_chunk_store(&chunks, &forward)?);
        let store_rev = ChunkStoreHandle::new(build_overlapping_chunk_store(&chunks, &reversed)?);
        let store_shuf = ChunkStoreHandle::new(build_overlapping_chunk_store(&chunks, &shuffled)?);

        let filtered_index_name = TimelineName::new("frame_nr");
        let filtered_index = Some(filtered_index_name);

        let make_query = |range: Option<AbsoluteTimeRange>| QueryExpression {
            filtered_index,
            filtered_index_range: range,
            selection: Some(vec![
                ColumnSelector::Time(TimeColumnSelector::from(filtered_index_name)),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: EntityPath::from("/overlap"),
                    component: MyPoints::descriptor_points().component.to_string(),
                }),
            ]),
            ..Default::default()
        };

        let collect_per_row_arrays = |store: &ChunkStoreHandle,
                                      query: QueryExpression|
         -> anyhow::Result<Vec<Vec<ArrayRef>>> {
            let cache = QueryCache::new_handle(store.clone());
            let engine = QueryEngine::new(store.clone(), cache);
            let mut handle = engine.query(query);
            let mut out = Vec::new();
            while let Some(row) = handle.next_row() {
                out.push(row);
            }
            Ok(out)
        };

        // Full-range and mid-range queries. Mid-range crosses chunk boundaries
        // to exercise H1 (exhausted), H2 (time-range prune), and H3 (early
        // break) jointly under overlap.
        //
        // Total unique frames: union of [0,5), [3,8), [6,11) = [0,11) -> 11 rows.
        // Mid-range [4..=7] picks 4 rows, with frame 4 in A∩B and frames 6,7 in B∩C.
        let full_query = make_query(None);
        let mid_query = make_query(Some(AbsoluteTimeRange::new(4, 7)));

        for (label, query, expected_rows) in [
            ("full-range", full_query, 11usize),
            ("mid-range", mid_query, 4usize),
        ] {
            // Batched path: byte-identical record batches across all 3 orderings.
            let rb_fwd = run_query_collect_rows(&store_fwd, query.clone())?;
            let rb_rev = run_query_collect_rows(&store_rev, query.clone())?;
            let rb_shuf = run_query_collect_rows(&store_shuf, query.clone())?;
            assert_eq!(rb_fwd.num_rows(), expected_rows, "{label}: row count");
            assert_eq!(rb_fwd, rb_rev, "{label}: batched fwd vs reversed");
            assert_eq!(rb_fwd, rb_shuf, "{label}: batched fwd vs shuffled");

            // Per-row path drives `_resolve_one_row` directly.
            let rows_fwd = collect_per_row_arrays(&store_fwd, query.clone())?;
            let rows_rev = collect_per_row_arrays(&store_rev, query.clone())?;
            let rows_shuf = collect_per_row_arrays(&store_shuf, query)?;
            assert_eq!(rows_fwd.len(), expected_rows, "{label}: per-row count");
            for (other_label, other) in [("reversed", &rows_rev), ("shuffled", &rows_shuf)] {
                assert_eq!(
                    rows_fwd.len(),
                    other.len(),
                    "{label}: per-row count diverged vs {other_label}",
                );
                for (i, (a_row, b_row)) in std::iter::zip(&rows_fwd, other).enumerate() {
                    assert_eq!(
                        a_row.len(),
                        b_row.len(),
                        "{label}: per-row column count diverged at row {i} vs {other_label}",
                    );
                    for (col_idx, (a, b)) in std::iter::zip(a_row, b_row).enumerate() {
                        assert_eq!(
                            a.to_data(),
                            b.to_data(),
                            "{label}: row {i} col {col_idx} changed vs {other_label}",
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Verifies that the H1/H2/H3 pruning logic produces identical row output regardless
    /// of the insertion order of disjoint chunks. The view-init sort by `time_min` (H3)
    /// must normalize the walk so that:
    /// - forward, reversed, and shuffled chunk inserts yield byte-identical record batches
    /// - mid-range filtered queries (which exercise H1 exhaustion of earlier chunks and
    ///   H3 early-break for later chunks) match the unfiltered slice.
    #[test]
    fn pruning_walk_insert_order_invariance() -> anyhow::Result<()> {
        re_log::setup_logging();

        const N_CHUNKS: usize = 5;
        const ROWS_PER_CHUNK: usize = 4;
        const TOTAL_ROWS: usize = N_CHUNKS * ROWS_PER_CHUNK;

        let forward: Vec<usize> = (0..N_CHUNKS).collect();
        let reversed: Vec<usize> = (0..N_CHUNKS).rev().collect();
        let shuffled: Vec<usize> = vec![2, 0, 4, 1, 3];

        let store_fwd = ChunkStoreHandle::new(build_disjoint_chunk_store(
            N_CHUNKS,
            ROWS_PER_CHUNK,
            &forward,
        )?);
        let store_rev = ChunkStoreHandle::new(build_disjoint_chunk_store(
            N_CHUNKS,
            ROWS_PER_CHUNK,
            &reversed,
        )?);
        let store_shuf = ChunkStoreHandle::new(build_disjoint_chunk_store(
            N_CHUNKS,
            ROWS_PER_CHUNK,
            &shuffled,
        )?);

        let filtered_index = Some(TimelineName::new("frame_nr"));

        // Full-range query: all rows visible. Each row should match across orderings.
        let full_query = QueryExpression {
            filtered_index,
            ..Default::default()
        };

        let rb_fwd = run_query_collect_rows(&store_fwd, full_query.clone())?;
        let rb_rev = run_query_collect_rows(&store_rev, full_query.clone())?;
        let rb_shuf = run_query_collect_rows(&store_shuf, full_query.clone())?;

        assert_eq!(rb_fwd.num_rows(), TOTAL_ROWS);
        assert_eq!(rb_fwd, rb_rev, "forward vs reversed insert order differ");
        assert_eq!(rb_fwd, rb_shuf, "forward vs shuffled insert order differ");

        // Mid-range query: indices [8..16) skip the first two chunks entirely (H1/H2)
        // and break out before reaching the last chunk (H3).
        let mid_query = QueryExpression {
            filtered_index,
            filtered_index_range: Some(AbsoluteTimeRange::new(8, 15)),
            ..Default::default()
        };

        let mid_fwd = run_query_collect_rows(&store_fwd, mid_query.clone())?;
        let mid_rev = run_query_collect_rows(&store_rev, mid_query.clone())?;
        let mid_shuf = run_query_collect_rows(&store_shuf, mid_query)?;

        assert_eq!(mid_fwd.num_rows(), 8); // frames 8..=15 inclusive
        assert_eq!(mid_fwd, mid_rev, "mid-range fwd vs reversed differ");
        assert_eq!(mid_fwd, mid_shuf, "mid-range fwd vs shuffled differ");

        // Sanity: per-row `next_row` matches batch output.
        let cache = QueryCache::new_handle(store_shuf.clone());
        let engine = QueryEngine::new(store_shuf.clone(), cache);
        let mut handle = engine.query(QueryExpression {
            filtered_index,
            ..Default::default()
        });
        let mut row_count = 0usize;
        while handle.next_row().is_some() {
            row_count += 1;
        }
        assert_eq!(row_count, TOTAL_ROWS);

        Ok(())
    }
}
