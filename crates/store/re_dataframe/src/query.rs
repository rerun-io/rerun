use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

use ahash::HashSet;
use arrow2::{
    array::{
        Array as ArrowArray, BooleanArray as ArrowBooleanArray,
        PrimitiveArray as ArrowPrimitiveArray,
    },
    chunk::Chunk as ArrowChunk,
    datatypes::Schema as ArrowSchema,
    Either,
};
use itertools::Itertools;
use parking_lot::Mutex;

use nohash_hasher::{IntMap, IntSet};
use re_chunk::{
    Chunk, ComponentName, EntityPath, RangeQuery, RowId, TimeInt, Timeline, UnitChunkShared,
};
use re_chunk_store::{
    ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor, ComponentColumnSelector,
    IndexValue, JoinEncoding, QueryExpression, SparseFillStrategy, TimeColumnDescriptor,
    TimeColumnSelector,
};
use re_log_types::ResolvedTimeRange;
use re_types_core::components::ClearIsRecursive;

use crate::{QueryEngine, RecordBatch};

// ---

// TODO(cmc): (no specific order) (should we make issues for these?)
// * [x] basic thing working
// * [x] custom selection
// * [x] support for overlaps (slow)
// * [x] pagination (any solution, even a slow one)
// * [x] pov support
// * [x] latestat sparse-filling
// * [x] sampling support
// * [x] clears
// * [ ] pagination (fast)
// * [ ] overlaps (less dumb)
// * [ ] selector-based `filtered_index`
// * [ ] configurable cache bypass
// * [ ] allocate null arrays once
// * [ ] take kernel duplicates all memory
// * [ ] dedupe-latest without allocs/copies

/// A handle to a dataframe query, ready to be executed.
///
/// Cheaply created via [`QueryEngine::query`].
///
/// See [`QueryHandle::next_row`] or [`QueryHandle::into_iter`].
pub struct QueryHandle<'a> {
    /// Handle to the [`QueryEngine`].
    pub(crate) engine: &'a QueryEngine<'a>,

    /// The original query expression used to instantiate this handle.
    pub(crate) query: QueryExpression,

    /// Internal private state. Lazily computed.
    ///
    /// It is important that handles stay cheap to create.
    state: OnceLock<QueryHandleState>,
}

/// Internal private state. Lazily computed.
struct QueryHandleState {
    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression::view_contents`].
    view_contents: Vec<ColumnDescriptor>,

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

    /// The Arrow schema that corresponds to the `selected_contents`.
    ///
    /// All returned rows will have this schema.
    arrow_schema: ArrowSchema,

    /// All the [`Chunk`]s included in the view contents.
    ///
    /// These are already sorted, densified, vertically sliced, and [latest-deduped] according
    /// to the query.
    ///
    /// The atomic counter is used as a cursor which keeps track of our current position within
    /// each individual chunk.
    /// Because chunks are allowed to overlap, we might need to rebound between two or more chunks
    /// during our iteration.
    ///
    /// This vector's entries correspond to those in [`QueryHandleState::view_contents`].
    /// Note: time and column entries don't have chunks -- inner vectors will be empty.
    ///
    /// [latest-deduped]: [`Chunk::deduped_latest_on_index`]
    //
    // NOTE: Reminder: we have to query everything in the _view_, irrelevant of the current selection.
    view_chunks: Vec<Vec<(AtomicU64, Chunk)>>,

    /// The index in `view_chunks` where the POV chunks are stored.
    ///
    /// * If set to `None`, then the caller didn't request a POV at all.
    /// * If set to `Some(usize::MAX)`, then a POV was requested but couldn't be found in the view.
    /// * Otherwise, points to the chunks that should be used to drive the iteration.
    view_pov_chunks_idx: Option<usize>,

    /// The stack of index values left to sample.
    ///
    /// Stored in reverse order, i.e. `pop()` is the next sample to retrieve.
    using_index_values_stack: Mutex<Option<Vec<IndexValue>>>,

    /// Tracks the current row index: the position of the iterator. For [`QueryHandle::next_row`].
    ///
    /// This represents the number of rows that the caller has iterated on: it is completely
    /// unrelated to the cursors used to track the current position in each individual chunk.
    cur_row: AtomicU64,
}

impl<'a> QueryHandle<'a> {
    pub(crate) fn new(engine: &'a QueryEngine<'a>, query: QueryExpression) -> Self {
        Self {
            engine,
            query,
            state: Default::default(),
        }
    }
}

impl QueryHandle<'_> {
    /// Lazily initialize internal private state.
    ///
    /// It is important that query handles stay cheap to create.
    fn init(&self) -> &QueryHandleState {
        self.state.get_or_init(|| self.init_())
    }

    // NOTE: This is split in its own method otherwise it completely breaks `rustfmt`.
    fn init_(&self) -> QueryHandleState {
        re_tracing::profile_scope!("init");

        // 1. Compute the schema of the view contents.
        let view_contents = if let Some(view_contents) = self.query.view_contents.as_ref() {
            self.engine.store.schema_for_view_contents(view_contents)
        } else {
            self.engine.store.schema()
        };

        // 2. Compute the schema of the selected contents.
        //
        // The caller might have selected columns that do not exist in the view: they should
        // still appear in the results.
        let selected_contents: Vec<(_, _)> = if let Some(selection) = self.query.selection.as_ref()
        {
            self.compute_user_selection(&view_contents, selection)
        } else {
            view_contents.clone().into_iter().enumerate().collect()
        };

        // 3. Compute the Arrow schema of the selected components.
        //
        // Every result returned using this `QueryHandle` will match this schema exactly.
        let arrow_schema = ArrowSchema {
            fields: selected_contents
                .iter()
                .map(|(_, descr)| descr.to_arrow_field())
                .collect_vec(),
            metadata: Default::default(),
        };

        let using_index_values_stack = self
            .query
            .using_index_values
            .as_ref()
            .map(|values| values.iter().rev().copied().collect_vec());

        // 4. Perform the query and keep track of all the relevant chunks.
        let query = {
            let index_range =
                if let Some(using_index_values_stack) = using_index_values_stack.as_ref() {
                    using_index_values_stack
                        .last() // reminder: it's reversed (stack)
                        .and_then(|start| using_index_values_stack.first().map(|end| (start, end)))
                        .map_or(ResolvedTimeRange::EMPTY, |(start, end)| {
                            ResolvedTimeRange::new(*start, *end)
                        })
                } else {
                    self.query
                        .filtered_index_range
                        .unwrap_or(ResolvedTimeRange::EVERYTHING)
                };

            RangeQuery::new(self.query.filtered_index, index_range)
                .keep_extra_timelines(true) // we want all the timelines we can get!
                .keep_extra_components(false)
        };
        let (view_pov_chunks_idx, mut view_chunks) = self.fetch_view_chunks(&query, &view_contents);

        // 5. Collect all relevant clear chunks and update the view accordingly.
        //
        // We'll turn the clears into actual empty arrays of the expected component type.
        {
            re_tracing::profile_scope!("clear_chunks");

            let clear_chunks = self.fetch_clear_chunks(&query, &view_contents);
            for (view_idx, chunks) in view_chunks.iter_mut().enumerate() {
                let Some(ColumnDescriptor::Component(descr)) = view_contents.get(view_idx) else {
                    continue;
                };

                // NOTE: It would be tempting to concatenate all these individual clear chunks into one
                // single big chunk, but that'd be a mistake: 1) it's costly to do so but more
                // importantly 2) that would lead to likely very large chunk overlap, which is very bad
                // for business.
                if let Some(clear_chunks) = clear_chunks.get(&descr.entity_path) {
                    chunks.extend(clear_chunks.iter().map(|chunk| {
                        let child_datatype = match &descr.store_datatype {
                            arrow2::datatypes::DataType::List(field)
                            | arrow2::datatypes::DataType::LargeList(field) => {
                                field.data_type().clone()
                            }
                            arrow2::datatypes::DataType::Dictionary(_, datatype, _) => {
                                (**datatype).clone()
                            }
                            datatype => datatype.clone(),
                        };

                        let mut chunk = chunk.clone();
                        // Only way this could fail is if the number of rows did not match.
                        #[allow(clippy::unwrap_used)]
                        chunk
                            .add_component(
                                descr.component_name,
                                re_chunk::util::new_list_array_of_empties(
                                    child_datatype,
                                    chunk.num_rows(),
                                ),
                            )
                            .unwrap();

                        (AtomicU64::new(0), chunk)
                    }));

                    // The chunks were sorted that way before, and it needs to stay that way after.
                    chunks.sort_by_key(|(_cursor, chunk)| {
                        // NOTE: The chunk has been densified already: its global time range is the same as
                        // the time range for the specific component of interest.
                        chunk
                            .timelines()
                            .get(&self.query.filtered_index)
                            .map(|time_column| time_column.time_range())
                            .map_or(TimeInt::STATIC, |time_range| time_range.min())
                    });
                }
            }
        }
            };

                        }

            }

        QueryHandleState {
            view_contents,
            selected_contents,
            arrow_schema,
            view_chunks,
            view_pov_chunks_idx,
            using_index_values_stack: Mutex::new(using_index_values_stack),
            cur_row: AtomicU64::new(0),
        }
    }

    #[allow(clippy::unused_self)]
    fn compute_user_selection(
        &self,
        view_contents: &[ColumnDescriptor],
        selection: &[ColumnSelector],
    ) -> Vec<(usize, ColumnDescriptor)> {
        selection
            .iter()
            .map(|column| {
                match column {
                    ColumnSelector::Time(selected_column) => {
                        let TimeColumnSelector {
                            timeline: selected_timeline,
                        } = selected_column;

                        view_contents
                            .iter()
                            .enumerate()
                            .filter_map(|(idx, view_column)| match view_column {
                                ColumnDescriptor::Time(view_descr) => Some((idx, view_descr)),
                                ColumnDescriptor::Component(_) => None,
                            })
                            .find(|(_idx, view_descr)| {
                                *view_descr.timeline.name() == *selected_timeline
                            })
                            .map_or_else(
                                || {
                                    (
                                        usize::MAX,
                                        ColumnDescriptor::Time(TimeColumnDescriptor {
                                            // TODO(cmc): I picked a sequence here because I have to pick something.
                                            // It doesn't matter, only the name will remain in the Arrow schema anyhow.
                                            timeline: Timeline::new_sequence(*selected_timeline),
                                            datatype: arrow2::datatypes::DataType::Null,
                                        }),
                                    )
                                },
                                |(idx, view_descr)| {
                                    (idx, ColumnDescriptor::Time(view_descr.clone()))
                                },
                            )
                    }

                    ColumnSelector::Component(selected_column) => {
                        let ComponentColumnSelector {
                            entity_path: selected_entity_path,
                            component: selected_component_name,
                            join_encoding: _,
                        } = selected_column;

                        view_contents
                            .iter()
                            .enumerate()
                            .filter_map(|(idx, view_column)| match view_column {
                                ColumnDescriptor::Component(view_descr) => Some((idx, view_descr)),
                                ColumnDescriptor::Time(_) => None,
                            })
                            .find(|(_idx, view_descr)| {
                                view_descr.entity_path == *selected_entity_path
                                    && view_descr.component_name == *selected_component_name
                            })
                            .map_or_else(
                                || {
                                    (
                                        usize::MAX,
                                        ColumnDescriptor::Component(ComponentColumnDescriptor {
                                            entity_path: selected_entity_path.clone(),
                                            archetype_name: None,
                                            archetype_field_name: None,
                                            component_name: *selected_component_name,
                                            store_datatype: arrow2::datatypes::DataType::Null,
                                            join_encoding: JoinEncoding::default(),
                                            is_static: false,
                                        }),
                                    )
                                },
                                |(idx, view_descr)| {
                                    (idx, ColumnDescriptor::Component(view_descr.clone()))
                                },
                            )
                    }
                }
            })
            .collect_vec()
    }

    fn fetch_view_chunks(
        &self,
        query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> (Option<usize>, Vec<Vec<(AtomicU64, Chunk)>>) {
        let mut view_pov_chunks_idx = self
            .query
            .filtered_point_of_view
            .as_ref()
            .map(|_| usize::MAX);

        let view_chunks = view_contents
            .iter()
            .enumerate()
            .map(|(idx, selected_column)| match selected_column {
                ColumnDescriptor::Time(_) => Vec::new(),

                ColumnDescriptor::Component(column) => {
                    let chunks = self
                        .fetch_chunks(query, &column.entity_path, [column.component_name])
                        .unwrap_or_default();

                    if let Some(pov) = self.query.filtered_point_of_view.as_ref() {
                        if pov.entity_path == column.entity_path
                            && pov.component == column.component_name
                        {
                            view_pov_chunks_idx = Some(idx);
                        }
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
        &self,
        query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> IntMap<EntityPath, Vec<Chunk>> {
        /// Returns all the ancestors of an [`EntityPath`].
        ///
        /// Doesn't return `entity_path` itself.
        fn entity_path_ancestors(entity_path: &EntityPath) -> impl Iterator<Item = EntityPath> {
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
            let list_array = chunk.components().get(&ClearIsRecursive::name())?;

            let values = list_array
                .values()
                .as_any()
                .downcast_ref::<ArrowBooleanArray>()?;

            let indices = ArrowPrimitiveArray::from_vec(
                values
                    .iter()
                    .enumerate()
                    .filter_map(|(index, is_recursive)| {
                        (is_recursive == Some(true)).then_some(index as i32)
                    })
                    .collect_vec(),
            );

            let chunk = chunk.taken(&indices);

            (!chunk.is_empty()).then_some(chunk)
        }

        use re_types_core::Loggable as _;
        let component_names = [re_types_core::components::ClearIsRecursive::name()];

        // All unique entity paths present in the view contents.
        let entity_paths: IntSet<EntityPath> = view_contents
            .iter()
            .filter_map(|col| match col {
                ColumnDescriptor::Component(descr) => Some(descr.entity_path.clone()),
                ColumnDescriptor::Time(_) => None,
            })
            .collect();

        entity_paths
            .iter()
            .filter_map(|entity_path| {
                // For the entity itself, any chunk that contains clear data is relevant, recursive or not.
                // Just fetch everything we find.
                let flat_chunks = self
                    .fetch_chunks(query, entity_path, component_names)
                    .map(|chunks| {
                        chunks
                            .into_iter()
                            .map(|(_cursor, chunk)| chunk)
                            .collect_vec()
                    })
                    .unwrap_or_default();

                let recursive_chunks =
                    entity_path_ancestors(entity_path).flat_map(|ancestor_path| {
                        self.fetch_chunks(query, &ancestor_path, component_names)
                            .into_iter() // option
                            .flat_map(|chunks| chunks.into_iter().map(|(_cursor, chunk)| chunk))
                            // NOTE: Ancestors' chunks are only relevant for the rows where `ClearIsRecursive=true`.
                            .filter_map(|chunk| chunk_filter_recursive_only(&chunk))
                    });

                let chunks = flat_chunks
                    .into_iter()
                    .chain(recursive_chunks)
                    // The component data is irrelevant.
                    // We do not expose the actual tombstones to end-users, only their _effect_.
                    .map(|chunk| chunk.components_removed())
                    .collect_vec();

                (!chunks.is_empty()).then(|| (entity_path.clone(), chunks))
            })
            .collect()
    }

    fn fetch_chunks<const N: usize>(
        &self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: [ComponentName; N],
    ) -> Option<Vec<(AtomicU64, Chunk)>> {
        // NOTE: Keep in mind that the range APIs natively make sure that we will
        // either get a bunch of relevant _static_ chunks, or a bunch of relevant
        // _temporal_ chunks, but never both.
        //
        // TODO(cmc): Going through the cache is very useful in a Viewer context, but
        // not so much in an SDK context. Make it configurable.
        let results =
            self.engine
                .cache
                .range(self.engine.store, query, entity_path, component_names);

        debug_assert!(
            results.components.len() <= 1,
            "cannot possibly get more than one component with this query"
        );

        results
            .components
            .into_iter()
            .next()
            .map(|(_component_name, chunks)| {
                chunks
                    .into_iter()
                    .map(|chunk| {
                        // NOTE: Keep in mind that the range APIs would have already taken care
                        // of A) sorting the chunk on the `filtered_index` (and row-id) and
                        // B) densifying it according to the current `component_name`.
                        // Both of these are mandatory requirements for the deduplication logic to
                        // do what we want: keep the latest known value for `component_name` at all
                        // remaining unique index values all while taking row-id ordering semantics
                        // into account.
                        debug_assert!(
                            chunk.is_sorted(),
                            "the query cache should have already taken care of sorting (and densifying!) the chunk",
                        );

                        let chunk = chunk.deduped_latest_on_index(&self.query.filtered_index);

                        (AtomicU64::default(), chunk)
                    })
                    .collect_vec()
            })
    }

    /// The query used to instantiate this handle.
    pub fn query(&self) -> &QueryExpression {
        &self.query
    }

    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression::view_contents`].
    pub fn view_contents(&self) -> &[ColumnDescriptor] {
        &self.init().view_contents
    }

    /// Describes the columns that make up this selection.
    ///
    /// The extra `usize` is the index in [`Self::view_contents`] that this selection points to.
    ///
    /// See [`QueryExpression::selection`].
    pub fn selected_contents(&self) -> &[(usize, ColumnDescriptor)] {
        &self.init().selected_contents
    }

    /// All results returned by this handle will strictly follow this Arrow schema.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    pub fn schema(&self) -> &ArrowSchema {
        &self.init().arrow_schema
    }

    /// How many rows of data will be returned?
    ///
    /// The number of rows depends and only depends on the _view contents_.
    /// The _selected contents_ has no influence on this value.
    //
    // TODO(cmc): implement this properly, cache the result, etc.
    pub fn num_rows(&self) -> u64 {
        re_tracing::profile_function!();

        let state = self.init();

        let mut view_chunks = state.view_chunks.iter();
        let view_chunks = if let Some(view_pov_chunks_idx) = state.view_pov_chunks_idx {
            Either::Left(view_chunks.nth(view_pov_chunks_idx).into_iter())
        } else {
            Either::Right(view_chunks)
        };

        let mut all_unique_timestamps: HashSet<TimeInt> = view_chunks
            .flat_map(|chunks| {
                chunks.iter().filter_map(|(_cursor, chunk)| {
                    if chunk.is_static() {
                        Some(Either::Left(std::iter::once(TimeInt::STATIC)))
                    } else {
                        chunk
                            .timelines()
                            .get(&self.query.filtered_index)
                            .map(|time_column| Either::Right(time_column.times()))
                    }
                })
            })
            .flatten()
            .collect();

        if let Some(filtered_index_values) = self.query.filtered_index_values.as_ref() {
            all_unique_timestamps.retain(|time| filtered_index_values.contains(time));
        }

        let num_rows = all_unique_timestamps.len() as _;

        if cfg!(debug_assertions) {
            let expected_num_rows =
                self.engine.query(self.query.clone()).into_iter().count() as u64;
            assert_eq!(expected_num_rows, num_rows);
        }

        num_rows
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
    /// This does not offer any kind of native pagination yet.
    ///
    /// To emulate pagination from user-space, use the `Iterator` API:
    /// ```ignore
    /// for row in query_handle.into_iter().skip(offset).take(len) {
    ///     // …
    /// }
    /// ```
    //
    // TODO(cmc): better/actual pagination
    pub fn next_row(&self) -> Option<Vec<Box<dyn ArrowArray>>> {
        re_tracing::profile_function!();

        /// Temporary state used to resolve the streaming join for the current iteration.
        #[derive(Debug)]
        struct StreamingJoinStateEntry<'a> {
            /// Which `Chunk` is this?
            chunk: &'a Chunk,

            /// How far are we into this `Chunk`?
            cursor: u64,

            /// What's the index value at the current cursor?
            index_value: TimeInt,

            /// What's the `RowId` at the current cursor?
            row_id: RowId,
        }

        /// Temporary state used to resolve the streaming join for the current iteration.
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

        let state = self.init();

        let _cur_row = state.cur_row.fetch_add(1, Ordering::Relaxed);

        // First, we need to find, among all the chunks available for the current view contents,
        // what is their index value for the current row?
        //
        // NOTE: Non-component columns don't have a streaming state, hence the optional layer.
        let mut view_streaming_state: Vec<Option<StreamingJoinStateEntry<'_>>> =
            // NOTE: cannot use vec![], it has limitations with non-cloneable options.
            // vec![None; state.view_chunks.len()];
            std::iter::repeat(())
                .map(|_| None)
                .take(state.view_chunks.len())
                .collect();
        for (view_column_idx, view_chunks) in state.view_chunks.iter().enumerate() {
            let streaming_state = &mut view_streaming_state[view_column_idx];

            for (cur_cursor, cur_chunk) in view_chunks {
                // NOTE: Too soon to increment the cursor, we cannot know yet which chunks will or
                // will not be part of the current row.
                let cur_cursor_value = cur_cursor.load(Ordering::Relaxed);

                // TODO(cmc): This can easily be optimized by looking ahead and breaking as soon as chunks
                // stop overlapping.

                let Some((cur_index_value, cur_row_id)) = cur_chunk
                    .iter_indices(&self.query.filtered_index)
                    .nth(cur_cursor_value as _)
                else {
                    continue;
                };

                if let Some(streaming_state) = streaming_state.as_mut() {
                    let StreamingJoinStateEntry {
                        chunk,
                        cursor,
                        index_value,
                        row_id,
                    } = streaming_state;

                    let cur_chunk_has_smaller_index_value = cur_index_value < *index_value;
                    // If these two chunks overlap and share the index value of the current
                    // iteration, we shall pick the row with the most recent row-id.
                    let cur_chunk_has_equal_index_but_higher_rowid =
                        cur_index_value == *index_value && cur_row_id > *row_id;

                    if cur_chunk_has_smaller_index_value
                        || cur_chunk_has_equal_index_but_higher_rowid
                    {
                        *chunk = cur_chunk;
                        *cursor = cur_cursor_value;
                        *index_value = cur_index_value;
                        *row_id = cur_row_id;
                    }
                } else {
                    *streaming_state = Some(StreamingJoinStateEntry {
                        chunk: cur_chunk,
                        cursor: cur_cursor_value,
                        index_value: cur_index_value,
                        row_id: cur_row_id,
                    });
                };
            }
        }

        // What's the index value we're looking for at the current iteration?
        //
        // `None` if we ran out of chunks.
        let mut cur_index_value = if let Some(view_pov_chunks_idx) = state.view_pov_chunks_idx {
            // If we do have a set point-of-view, then the current iteration corresponds to the
            // next available index value across all PoV chunks.
            view_streaming_state
                .get(view_pov_chunks_idx)
                .and_then(|streaming_state| streaming_state.as_ref().map(|s| s.index_value))
        } else {
            // If we do not have a set point-of-view, then the current iteration corresponds to the
            // smallest available index value across all available chunks.
            view_streaming_state
                .iter()
                .flatten()
                // NOTE: We're purposefully ignoring RowId-related semantics here: we just want to know
                // the value we're looking for on the "main" index (dedupe semantics).
                .min_by_key(|streaming_state| streaming_state.index_value)
                .map(|streaming_state| streaming_state.index_value)
        };

        // What's the next sample we're interested in, if any?
        //
        // `None` iff the query isn't sampled. Short-circuits if the query is sampled but has run out of samples.
        let sampled_index_value =
            if let Some(using_index_values) = state.using_index_values_stack.lock().as_mut() {
                // NOTE: Look closely -- this short-circuits is there are no more samples in the vec.
                Some(using_index_values.last().copied()?)
            } else {
                None
            };

        if let Some(sampled_index_value) = sampled_index_value {
            if let Some(cur_index_value) = cur_index_value {
                // If the current natural index value is smaller than the next sampled index value, then we need
                // to skip the current row.
                if sampled_index_value > cur_index_value {
                    self.increment_cursors_at_index_value(cur_index_value);
                    return self.next_row();
                }
            }

            // If the current natural index value is equal to the current sample, then we should
            // return the current row as usual.
            //
            // If the current natural index value is greater than the current sample, or if we wan out of
            // natural indices altogether, then we should return whatever data is available for the
            // current row (which might be all 'nulls').
            //
            // Either way, we can pop the sample from the stack, and use that as index value.

            // These unwraps cannot fail since we the only way to get here is for `state.using_index_values`
            // to exist and be non-empty.
            #[allow(clippy::unwrap_used)]
            {
                cur_index_value = Some(
                    state
                        .using_index_values_stack
                        .lock()
                        .as_mut()
                        .unwrap()
                        .pop()
                        .unwrap(),
                );
            }
        }

        let cur_index_value = cur_index_value?;

        // `filtered_index_values` & `using_index_values` are exclusive.
        if sampled_index_value.is_none() {
            if let Some(filtered_index_values) = self.query.filtered_index_values.as_ref() {
                if !filtered_index_values.contains(&cur_index_value) {
                    self.increment_cursors_at_index_value(cur_index_value);
                    return self.next_row();
                }
            }
        }

        let mut view_streaming_state = view_streaming_state
            .into_iter()
            .map(|streaming_state| match streaming_state {
                Some(s) if s.index_value == cur_index_value => {
                    Some(StreamingJoinState::StreamingJoinState(s))
                }
                _ => None,
            })
            .collect_vec();

        match self.query.sparse_fill_strategy {
            SparseFillStrategy::None => {}
            SparseFillStrategy::LatestAtGlobal => {
                // Everything that yielded `null` for the current iteration.
                let null_streaming_states = view_streaming_state
                    .iter_mut()
                    .enumerate()
                    .filter(|(_view_idx, streaming_state)| streaming_state.is_none());

                for (view_idx, streaming_state) in null_streaming_states {
                    let Some(ColumnDescriptor::Component(descr)) =
                        state.view_contents.get(view_idx)
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
                        re_chunk::LatestAtQuery::new(self.query.filtered_index, cur_index_value);

                    let results = self.engine.cache.latest_at(
                        self.engine.store,
                        &query,
                        &descr.entity_path,
                        [descr.component_name],
                    );

                    *streaming_state = results
                        .components
                        .get(&descr.component_name)
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
        let mut max_value_per_index = IntMap::default();
        {
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
                                    (time, time_column.times_array().sliced(cursor, 1)),
                                )
                            })
                    })
                })
                .for_each(|(timeline, (time, time_sliced))| {
                    max_value_per_index
                        .entry(timeline)
                        .and_modify(|(max_time, max_time_sliced)| {
                            if time > *max_time {
                                *max_time = time;
                                *max_time_sliced = time_sliced.clone();
                            }
                        })
                        .or_insert((time, time_sliced));
                });

            if let Some(sampled_index_value) = sampled_index_value {
                if !sampled_index_value.is_static() {
                    // The sampled index value (if temporal) should be the one returned for the
                    // queried index, no matter what.
                    max_value_per_index.insert(
                        self.query.filtered_index,
                        (
                            sampled_index_value,
                            ArrowPrimitiveArray::<i64>::from_vec(vec![cur_index_value.as_i64()])
                                .to(self.query.filtered_index.datatype())
                                .to_boxed(),
                        ),
                    );
                }
            }
        }

        // NOTE: Non-component entries have no data to slice, hence the optional layer.
        //
        // TODO(cmc): no point in slicing arrays that are not selected.
        let view_sliced_arrays: Vec<Option<_>> = view_streaming_state
            .iter()
            .enumerate()
            .map(|(view_idx, streaming_state)| {
                // NOTE: Reminder: the only reason the streaming state could be `None` here is
                // because this column does not have data for the current index value (i.e. `null`).
                streaming_state.as_ref().and_then(|streaming_state| {
                    let list_array = match streaming_state {
                        StreamingJoinState::StreamingJoinState(s) => {
                            debug_assert!(
                                s.chunk.components().len() <= 1,
                                "cannot possibly get more than one component with this query"
                            );

                            s.chunk
                                .components()
                                .first_key_value()
                                .map(|(_, list_array)| list_array.sliced(s.cursor as usize, 1))

                        }

                        StreamingJoinState::Retrofilled(unit) => {
                            let component_name = state.view_contents.get(view_idx).and_then(|col| match col {
                                ColumnDescriptor::Component(descr) => Some(descr.component_name),
                                ColumnDescriptor::Time(_) => None,
                            })?;
                            unit.components().get(&component_name).map(|list_array| list_array.to_boxed())
                        }
                    };


                    debug_assert!(
                        list_array.is_some(),
                        "This must exist or the chunk wouldn't have been sliced/retrofilled to start with."
                    );

                    // NOTE: This cannot possibly return None, see assert above.
                    list_array
                })
            })
            .collect();

        // TODO(cmc): It would likely be worth it to allocate all these possible
        // null-arrays ahead of time, and just return a pointer to those in the failure
        // case here.
        let selected_arrays = state
            .selected_contents
            .iter()
            .map(|(view_idx, column)| match column {
                ColumnDescriptor::Time(descr) => {
                    max_value_per_index.get(&descr.timeline).map_or_else(
                        || arrow2::array::new_null_array(column.datatype(), 1),
                        |(_time, time_sliced)| time_sliced.clone(),
                    )
                }

                ColumnDescriptor::Component(_descr) => view_sliced_arrays
                    .get(*view_idx)
                    .cloned()
                    .flatten()
                    .unwrap_or_else(|| arrow2::array::new_null_array(column.datatype(), 1)),
            })
            .collect_vec();

        self.increment_cursors_at_index_value(cur_index_value);

        debug_assert_eq!(state.arrow_schema.fields.len(), selected_arrays.len());

        Some(selected_arrays)
    }

    /// Calls [`Self::next_row`] and wraps the result in a [`RecordBatch`].
    ///
    /// Only use this if you absolutely need a [`RecordBatch`] as this adds a lot of allocation
    /// overhead.
    ///
    /// See [`Self::next_row`] for more information.
    #[inline]
    pub fn next_row_batch(&self) -> Option<RecordBatch> {
        Some(RecordBatch {
            schema: self.schema().clone(),
            data: ArrowChunk::new(self.next_row()?),
        })
    }

    /// Increment cursors for iteration corresponding to `cur_index_value`.
    //
    // NOTE: This is trickier than it looks: cursors need to be incremented not only for chunks
    // that were used to return data during the current iteration, but also chunks that
    // _attempted_ to return data and were preempted for one reason or another (overlap,
    // intra-timestamp tie-break, etc).
    fn increment_cursors_at_index_value(&self, cur_index_value: IndexValue) {
        let state = self.init();
        for view_chunks in &state.view_chunks {
            for (cur_cursor, cur_chunk) in view_chunks {
                if let Some((index_value, _row_id)) = cur_chunk
                    .iter_indices(&self.query.filtered_index)
                    .nth(cur_cursor.load(Ordering::Relaxed) as _)
                {
                    if cur_index_value == index_value {
                        cur_cursor.fetch_add(1, Ordering::Relaxed);
                    }
                };
            }
        }
    }
}

impl<'a> QueryHandle<'a> {
    /// Returns an iterator backed by [`Self::next_row`].
    #[allow(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(self) -> impl Iterator<Item = Vec<Box<dyn ArrowArray>>> + 'a {
        std::iter::from_fn(move || self.next_row())
    }

    /// Returns an iterator backed by [`Self::next_row_batch`].
    #[allow(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_batch_iter(self) -> impl Iterator<Item = RecordBatch> + 'a {
        std::iter::from_fn(move || self.next_row_batch())
    }
}

// ---

#[cfg(test)]
#[allow(clippy::iter_on_single_items)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
    use re_chunk_store::{ChunkStore, ChunkStoreConfig, ResolvedTimeRange, TimeInt};
    use re_log_types::{
        build_frame_nr, build_log_time,
        example_components::{MyColor, MyLabel, MyPoint},
        EntityPath, Timeline,
    };
    use re_types::components::ClearIsRecursive;
    use re_types_core::Loggable as _;

    use crate::QueryCache;

    use super::*;

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
    // * [x] filtered_point_of_view
    // * [x] sparse_fill_strategy
    // * [x] using_index_values
    //
    // In addition to those, some much needed extras:
    // * [x] num_rows
    // * [x] clears
    // * [ ] timelines returned with selection=none
    // * [ ] pagination

    // TODO(cmc): At some point I'd like to stress multi-entity queries too, but that feels less
    // urgent considering how things are implemented (each entity lives in its own index, so it's
    // really just more of the same).

    /// All features disabled.
    #[test]
    fn barebones() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let query = QueryExpression::new(timeline);
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concatenate_record_batches(
            query_handle.schema().clone(),
            &query_handle.into_batch_iter().collect_vec(),
        );
        eprintln!("{dataframe}");

        let got = format!("{:#?}", dataframe.data.iter().collect_vec());
        let expected = unindent::unindent(
            "\
            [
                Int64[None, 10, 20, 30, 40, 50, 60, 70],
                Timestamp(Nanosecond, None)[None, 1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000050, None, 1970-01-01 00:00:00.000000070],
                ListArray[None, None, None, [2], [3], [4], None, [6]],
                ListArray[[c], None, None, None, None, None, None, None],
                ListArray[None, [{x: 0, y: 0}], [{x: 1, y: 1}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [{x: 5, y: 5}], [{x: 8, y: 8}]],
            ]\
            "
        );

        similar_asserts::assert_eq!(expected, got);

        Ok(())
    }

    #[test]
    fn sparse_fill_strategy_latestatglobal() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let mut query = QueryExpression::new(timeline);
        query.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concatenate_record_batches(
            query_handle.schema().clone(),
            &query_handle.into_batch_iter().collect_vec(),
        );
        eprintln!("{dataframe}");

        let got = format!("{:#?}", dataframe.data.iter().collect_vec());
        let expected = unindent::unindent(
            "\
            [
                Int64[None, 10, 20, 30, 40, 50, 60, 70],
                Timestamp(Nanosecond, None)[None, 1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000050, None, 1970-01-01 00:00:00.000000070],
                ListArray[None, None, None, [2], [3], [4], [4], [6]],
                ListArray[[c], [c], [c], [c], [c], [c], [c], [c]],
                ListArray[None, [{x: 0, y: 0}], [{x: 1, y: 1}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [{x: 5, y: 5}], [{x: 8, y: 8}]],
            ]\
            "
        );

        similar_asserts::assert_eq!(expected, got);

        Ok(())
    }

    #[test]
    fn filtered_index_range() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let mut query = QueryExpression::new(timeline);
        query.filtered_index_range = Some(ResolvedTimeRange::new(30, 60));
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concatenate_record_batches(
            query_handle.schema().clone(),
            &query_handle.into_batch_iter().collect_vec(),
        );
        eprintln!("{dataframe}");

        let got = format!("{:#?}", dataframe.data.iter().collect_vec());
        let expected = unindent::unindent(
            "\
            [
                Int64[None, 30, 40, 50, 60],
                Timestamp(Nanosecond, None)[None, None, None, 1970-01-01 00:00:00.000000050, None],
                ListArray[None, [2], [3], [4], None],
                ListArray[[c], None, None, None, None],
                ListArray[None, [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [{x: 5, y: 5}]],
            ]\
            ",
        );

        similar_asserts::assert_eq!(expected, got);

        Ok(())
    }

    #[test]
    fn filtered_index_values() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let mut query = QueryExpression::new(timeline);
        query.filtered_index_values = Some(
            [0, 30, 60, 90]
                .into_iter()
                .map(TimeInt::new_temporal)
                .chain(std::iter::once(TimeInt::STATIC))
                .collect(),
        );
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concatenate_record_batches(
            query_handle.schema().clone(),
            &query_handle.into_batch_iter().collect_vec(),
        );
        eprintln!("{dataframe}");

        let got = format!("{:#?}", dataframe.data.iter().collect_vec());
        let expected = unindent::unindent(
            "\
            [
                Int64[None, 30, 60],
                Timestamp(Nanosecond, None)[None, None, None],
                ListArray[None, [2], None],
                ListArray[[c], None, None],
                ListArray[None, [{x: 2, y: 2}], [{x: 5, y: 5}]],
            ]\
            ",
        );

        similar_asserts::assert_eq!(expected, got);

        Ok(())
    }

    #[test]
    fn using_index_values() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");

        // vanilla
        {
            let mut query = QueryExpression::new(timeline);
            query.using_index_values = Some(
                [0, 15, 30, 30, 45, 60, 75, 90]
                    .into_iter()
                    .map(TimeInt::new_temporal)
                    .chain(std::iter::once(TimeInt::STATIC))
                    .collect(),
            );
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[None, 0, 15, 30, 45, 60, 75, 90],
                    Timestamp(Nanosecond, None)[None, None, None, None, None, None, None, None],
                    ListArray[None, None, None, [2], None, None, None, None],
                    ListArray[[c], None, None, None, None, None, None, None],
                    ListArray[None, None, None, [{x: 2, y: 2}], None, [{x: 5, y: 5}], None, None],
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        // sparse-filled
        if true {
            let mut query = QueryExpression::new(timeline);
            query.using_index_values = Some(
                [0, 15, 30, 30, 45, 60, 75, 90]
                    .into_iter()
                    .map(TimeInt::new_temporal)
                    .chain(std::iter::once(TimeInt::STATIC))
                    .collect(),
            );
            query.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[None, 0, 15, 30, 45, 60, 75, 90],
                    Timestamp(Nanosecond, None)[None, None, 1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000070, 1970-01-01 00:00:00.000000070],
                    ListArray[None, None, None, [2], [3], [4], [6], [6]],
                    ListArray[[c], [c], [c], [c], [c], [c], [c], [c]],
                    ListArray[None, None, [{x: 0, y: 0}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 5, y: 5}], [{x: 8, y: 8}], [{x: 8, y: 8}]],
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn filtered_point_of_view() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let entity_path: EntityPath = "this/that".into();

        // non-existing entity
        {
            let mut query = QueryExpression::new(timeline);
            query.filtered_point_of_view = Some(ComponentColumnSelector {
                entity_path: "no/such/entity".into(),
                component: MyPoint::name(),
                join_encoding: Default::default(),
            });
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = "[]";

            similar_asserts::assert_eq!(expected, got);
        }

        // non-existing component
        {
            let mut query = QueryExpression::new(timeline);
            query.filtered_point_of_view = Some(ComponentColumnSelector {
                entity_path: entity_path.clone(),
                component: "AComponentColumnThatDoesntExist".into(),
                join_encoding: Default::default(),
            });
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = "[]";

            similar_asserts::assert_eq!(expected, got);
        }

        // MyPoint
        {
            let mut query = QueryExpression::new(timeline);
            query.filtered_point_of_view = Some(ComponentColumnSelector {
                entity_path: entity_path.clone(),
                component: MyPoint::name(),
                join_encoding: Default::default(),
            });
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[10, 20, 30, 40, 50, 60, 70],
                    Timestamp(Nanosecond, None)[1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000050, None, 1970-01-01 00:00:00.000000070],
                    ListArray[None, None, [2], [3], [4], None, [6]],
                    ListArray[None, None, None, None, None, None, None],
                    ListArray[[{x: 0, y: 0}], [{x: 1, y: 1}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [{x: 5, y: 5}], [{x: 8, y: 8}]],
                ]\
                "
            );

            similar_asserts::assert_eq!(expected, got);
        }

        // MyColor
        {
            let mut query = QueryExpression::new(timeline);
            query.filtered_point_of_view = Some(ComponentColumnSelector {
                entity_path: entity_path.clone(),
                component: MyColor::name(),
                join_encoding: Default::default(),
            });
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[30, 40, 50, 70],
                    Timestamp(Nanosecond, None)[None, None, None, None],
                    ListArray[[2], [3], [4], [6]],
                    ListArray[None, None, None, None],
                    ListArray[None, None, None, None],
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn view_contents() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let entity_path: EntityPath = "this/that".into();
        let timeline = Timeline::new_sequence("frame_nr");

        // empty view
        {
            let mut query = QueryExpression::new(timeline);
            query.view_contents = Some(
                [(entity_path.clone(), Some(Default::default()))]
                    .into_iter()
                    .collect(),
            );
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = "[]";

            similar_asserts::assert_eq!(expected, got);
        }

        {
            let mut query = QueryExpression::new(timeline);
            query.view_contents = Some(
                [(
                    entity_path.clone(),
                    Some(
                        [
                            MyLabel::name(),
                            MyColor::name(),
                            "AColumnThatDoesntEvenExist".into(),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            );
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[None, 30, 40, 50, 70],
                    Timestamp(Nanosecond, None)[None, None, None, None, None],
                    ListArray[None, [2], [3], [4], [6]],
                    ListArray[[c], None, None, None, None],
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn selection() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let entity_path: EntityPath = "this/that".into();
        let timeline = Timeline::new_sequence("frame_nr");

        // empty selection
        {
            let mut query = QueryExpression::new(timeline);
            query.selection = Some(vec![]);
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = "[]";

            similar_asserts::assert_eq!(expected, got);
        }

        // only indices (+ duplication)
        {
            let mut query = QueryExpression::new(timeline);
            query.selection = Some(vec![
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: *timeline.name(),
                }),
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: *timeline.name(),
                }),
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: "ATimeColumnThatDoesntExist".into(),
                }),
            ]);
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[None, 10, 20, 30, 40, 50, 60, 70],
                    Int64[None, 10, 20, 30, 40, 50, 60, 70],
                    NullArray(8),
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        // only components (+ duplication)
        {
            let mut query = QueryExpression::new(timeline);
            query.selection = Some(vec![
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: MyColor::name(),
                    join_encoding: Default::default(),
                }),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: MyColor::name(),
                    join_encoding: Default::default(),
                }),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: "non_existing_entity".into(),
                    component: MyColor::name(),
                    join_encoding: Default::default(),
                }),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: "AComponentColumnThatDoesntExist".into(),
                    join_encoding: Default::default(),
                }),
            ]);
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    ListArray[None, None, None, [2], [3], [4], None, [6]],
                    ListArray[None, None, None, [2], [3], [4], None, [6]],
                    NullArray(8),
                    NullArray(8),
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn view_contents_and_selection() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let entity_path: EntityPath = "this/that".into();
        let timeline = Timeline::new_sequence("frame_nr");

        // only components
        {
            let mut query = QueryExpression::new(timeline);
            query.view_contents = Some(
                [(
                    entity_path.clone(),
                    Some([MyColor::name(), MyLabel::name()].into_iter().collect()),
                )]
                .into_iter()
                .collect(),
            );
            query.selection = Some(vec![
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: *timeline.name(),
                }),
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: *Timeline::log_time().name(),
                }),
                ColumnSelector::Time(TimeColumnSelector {
                    timeline: *Timeline::log_tick().name(),
                }),
                //
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: MyPoint::name(),
                    join_encoding: Default::default(),
                }),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: MyColor::name(),
                    join_encoding: Default::default(),
                }),
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: entity_path.clone(),
                    component: MyLabel::name(),
                    join_encoding: Default::default(),
                }),
            ]);
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
                "\
                [
                    Int64[None, 30, 40, 50, 70],
                    Timestamp(Nanosecond, None)[None, None, None, None, None],
                    NullArray(5),
                    NullArray(5),
                    ListArray[None, [2], [3], [4], [6]],
                    ListArray[[c], None, None, None, None],
                ]\
                ",
            );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn clears() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = create_nasty_store()?;
        extend_nasty_store_with_clears(&mut store)?;
        eprintln!("{store}");

        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let entity_path = EntityPath::from("this/that");

        // barebones
        {
            let mut query = QueryExpression::new(timeline);
            query.view_contents = Some([(entity_path.clone(), None)].into_iter().collect());
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
            "\
            [
                Int64[None, 10, 20, 30, 40, 50, 60, 65, 70],
                Timestamp(Nanosecond, None)[None, 1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000050, 1970-01-01 00:00:00.000000060, 1970-01-01 00:00:00.000000065, 1970-01-01 00:00:00.000000070],
                ListArray[[], None, None, [2], [3], [4], [], [], [6]],
                ListArray[[], None, None, None, None, None, [], [], None],
                ListArray[[], [{x: 0, y: 0}], [{x: 1, y: 1}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [], [], [{x: 8, y: 8}]],
            ]\
            "
        );

            similar_asserts::assert_eq!(expected, got);
        }

        // sparse-filled
        {
            let mut query = QueryExpression::new(timeline);
            query.view_contents = Some([(entity_path.clone(), None)].into_iter().collect());
            query.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concatenate_record_batches(
                query_handle.schema().clone(),
                &query_handle.into_batch_iter().collect_vec(),
            );
            eprintln!("{dataframe}");

            // TODO(#7650): Those null values for `MyColor` on 10 and 20 look completely insane, but then again
            // static clear semantics in general are pretty unhinged right now, especially when
            // ranges are involved.
            // It's extremely niche, our time is better spent somewhere else right now.
            let got = format!("{:#?}", dataframe.data.iter().collect_vec());
            let expected = unindent::unindent(
            "\
            [
                Int64[None, 10, 20, 30, 40, 50, 60, 65, 70],
                Timestamp(Nanosecond, None)[None, 1970-01-01 00:00:00.000000010, None, None, None, 1970-01-01 00:00:00.000000050, 1970-01-01 00:00:00.000000060, 1970-01-01 00:00:00.000000065, 1970-01-01 00:00:00.000000070],
                ListArray[[], None, None, [2], [3], [4], [], [], [6]],
                ListArray[[], [c], [c], [c], [c], [c], [], [], [c]],
                ListArray[[], [{x: 0, y: 0}], [{x: 1, y: 1}], [{x: 2, y: 2}], [{x: 3, y: 3}], [{x: 4, y: 4}], [], [], [{x: 8, y: 8}]],
            ]\
            "
        );

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    /// Returns a very nasty [`ChunkStore`] with all kinds of partial updates, chunk overlaps,
    /// repeated timestamps, duplicated chunks, partial multi-timelines, flat and recursive clears, etc.
    fn create_nasty_store() -> anyhow::Result<ChunkStore> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
                    (MyPoint::name(), Some(&points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), Some(&labels1 as _)), // shadowed by static
                ],
            )
            .with_sparse_component_batches(
                row_id1_3,
                [build_frame_nr(frame3), build_log_time(frame3.into())],
                [
                    (MyPoint::name(), Some(&points3 as _)),
                    (MyColor::name(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id1_5,
                [build_frame_nr(frame5), build_log_time(frame5.into())],
                [
                    (MyPoint::name(), Some(&points5 as _)),
                    (MyColor::name(), None),
                ],
            )
            .with_sparse_component_batches(
                row_id1_7_1,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoint::name(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_2,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoint::name(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_3,
                [build_frame_nr(frame7), build_log_time(frame7.into())],
                [(MyPoint::name(), Some(&points7_3 as _))],
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
                [(MyPoint::name(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                row_id2_3,
                [build_frame_nr(frame3)],
                [
                    (MyPoint::name(), Some(&points3 as _)),
                    (MyColor::name(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2_4,
                [build_frame_nr(frame4)],
                [(MyPoint::name(), Some(&points4 as _))],
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
                [(MyPoint::name(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                row_id3_4,
                [build_frame_nr(frame4)],
                [(MyPoint::name(), Some(&points4 as _))],
            )
            .with_sparse_component_batches(
                row_id3_6,
                [build_frame_nr(frame6)],
                [(MyPoint::name(), Some(&points6 as _))],
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
                [(MyColor::name(), Some(&colors4 as _))],
            )
            .with_sparse_component_batches(
                row_id4_5,
                [build_frame_nr(frame5)],
                [(MyColor::name(), Some(&colors5 as _))],
            )
            .with_sparse_component_batches(
                row_id4_7,
                [build_frame_nr(frame7)],
                [(MyColor::name(), Some(&colors7 as _))],
            )
            .build()?;

        let chunk4 = Arc::new(chunk4);
        store.insert_chunk(&chunk4)?;

        let row_id5_1 = RowId::new();
        let chunk5 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id5_1,
                TimePoint::default(),
                [(MyLabel::name(), Some(&labels2 as _))],
            )
            .build()?;

        let chunk5 = Arc::new(chunk5);
        store.insert_chunk(&chunk5)?;

        let row_id6_1 = RowId::new();
        let chunk6 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id6_1,
                TimePoint::default(),
                [(MyLabel::name(), Some(&labels3 as _))],
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

        let clear_flat = ClearIsRecursive(false.into());
        let clear_recursive = ClearIsRecursive(true.into());

        let row_id1_1 = RowId::new();
        let chunk1 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id1_1,
                TimePoint::default(),
                [(ClearIsRecursive::name(), Some(&clear_flat as _))],
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
                [(ClearIsRecursive::name(), Some(&clear_recursive as _))],
            )
            .build()?;

        let chunk2 = Arc::new(chunk2);
        store.insert_chunk(&chunk2)?;

        let row_id3_1 = RowId::new();
        let chunk3 = Chunk::builder(entity_path_root.clone())
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame55), build_log_time(frame55.into())],
                [(ClearIsRecursive::name(), Some(&clear_flat as _))],
            )
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame60), build_log_time(frame60.into())],
                [(ClearIsRecursive::name(), Some(&clear_recursive as _))],
            )
            .with_sparse_component_batches(
                row_id3_1,
                [build_frame_nr(frame65), build_log_time(frame65.into())],
                [(ClearIsRecursive::name(), Some(&clear_flat as _))],
            )
            .build()?;

        let chunk3 = Arc::new(chunk3);
        store.insert_chunk(&chunk3)?;

        let row_id4_1 = RowId::new();
        let chunk4 = Chunk::builder(entity_path_parent.clone())
            .with_sparse_component_batches(
                row_id4_1,
                [build_frame_nr(frame60), build_log_time(frame60.into())],
                [(ClearIsRecursive::name(), Some(&clear_flat as _))],
            )
            .build()?;

        let chunk4 = Arc::new(chunk4);
        store.insert_chunk(&chunk4)?;

        let row_id5_1 = RowId::new();
        let chunk5 = Chunk::builder(entity_path_parent.clone())
            .with_sparse_component_batches(
                row_id5_1,
                [build_frame_nr(frame65), build_log_time(frame65.into())],
                [(ClearIsRecursive::name(), Some(&clear_recursive as _))],
            )
            .build()?;

        let chunk5 = Arc::new(chunk5);
        store.insert_chunk(&chunk5)?;

        Ok(())
    }

    fn concatenate_record_batches(schema: ArrowSchema, batches: &[RecordBatch]) -> RecordBatch {
        assert!(batches.iter().map(|batch| &batch.schema).all_equal());

        let mut arrays = Vec::new();

        if !batches.is_empty() {
            for (i, _field) in schema.fields.iter().enumerate() {
                let array = arrow2::compute::concatenate::concatenate(
                    &batches
                        .iter()
                        .map(|batch| &*batch.data[i] as &dyn ArrowArray)
                        .collect_vec(),
                )
                .unwrap();
                arrays.push(array);
            }
        }

        RecordBatch {
            schema,
            data: ArrowChunk::new(arrays),
        }
    }
}
