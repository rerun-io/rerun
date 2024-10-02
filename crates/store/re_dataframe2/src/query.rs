use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

use ahash::HashSet;
use arrow2::{
    array::Array as ArrowArray, chunk::Chunk as ArrowChunk, datatypes::Schema as ArrowSchema,
};
use itertools::Itertools;

use nohash_hasher::IntMap;
use re_chunk::{Chunk, RangeQuery, RowId, TimeInt, Timeline, UnitChunkShared};
use re_chunk_store::{
    ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor, ComponentColumnSelector,
    ControlColumnDescriptor, ControlColumnSelector, JoinEncoding, QueryExpression2,
    TimeColumnDescriptor, TimeColumnSelector,
};
use re_log_types::ResolvedTimeRange;

use crate::{QueryEngine, RecordBatch};

// ---

// TODO(cmc): (no specific order) (should we make issues for these?)
// * [x] basic thing working
// * [x] custom selection
// * [x] support for overlaps (slow)
// * [x] pagination (any solution, even a slow one)
// * [x] latestat sparse-filling
// * [ ] overlaps (less dumb)
// * [ ] selector-based `filtered_index`
// * [ ] clears
// * [ ] pagination (fast)
// * [ ] pov support
// * [ ] sampling support
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
    pub(crate) query: QueryExpression2,

    /// Internal private state. Lazily computed.
    ///
    /// It is important that handles stay cheap to create.
    state: OnceLock<QueryHandleState>,
}

/// Internal private state. Lazily computed.
struct QueryHandleState {
    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression2::view_contents`].
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

    /// Tracks the current row index: the position of the iterator. For [`QueryHandle::next_row`].
    ///
    /// This represents the number of rows that the caller has iterated on: it is completely
    /// unrelated to the cursors used to track the current position in each individual chunk.
    cur_row: AtomicU64,
}

impl<'a> QueryHandle<'a> {
    pub(crate) fn new(engine: &'a QueryEngine<'a>, query: QueryExpression2) -> Self {
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
            selection
                .iter()
                .map(|column| {
                    match column {
                        ColumnSelector::Control(selected_column) => {
                            let ControlColumnSelector {
                                component: selected_component_name,
                            } = selected_column;

                            view_contents
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, view_column)| match view_column {
                                    ColumnDescriptor::Control(view_descr) => {
                                        Some((idx, view_descr))
                                    }
                                    _ => None,
                                })
                                .find(|(_idx, view_descr)| {
                                    view_descr.component_name == *selected_component_name
                                })
                                .map_or_else(
                                    || {
                                        (
                                            usize::MAX,
                                            ColumnDescriptor::Control(ControlColumnDescriptor {
                                                component_name: *selected_component_name,
                                                datatype: arrow2::datatypes::DataType::Null,
                                            }),
                                        )
                                    },
                                    |(idx, view_descr)| {
                                        (idx, ColumnDescriptor::Control(view_descr.clone()))
                                    },
                                )
                        }

                        ColumnSelector::Time(selected_column) => {
                            let TimeColumnSelector {
                                timeline: selected_timeline,
                            } = selected_column;

                            view_contents
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, view_column)| match view_column {
                                    ColumnDescriptor::Time(view_descr) => Some((idx, view_descr)),
                                    _ => None,
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
                                                timeline: Timeline::new_sequence(
                                                    *selected_timeline,
                                                ),
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
                                    ColumnDescriptor::Component(view_descr) => {
                                        Some((idx, view_descr))
                                    }
                                    _ => None,
                                })
                                .find(|(_idx, view_descr)| {
                                    view_descr.entity_path == *selected_entity_path
                                        && view_descr.component_name == *selected_component_name
                                })
                                .map_or_else(
                                    || {
                                        (
                                            usize::MAX,
                                            ColumnDescriptor::Component(
                                                ComponentColumnDescriptor {
                                                    entity_path: selected_entity_path.clone(),
                                                    archetype_name: None,
                                                    archetype_field_name: None,
                                                    component_name: *selected_component_name,
                                                    store_datatype:
                                                        arrow2::datatypes::DataType::Null,
                                                    join_encoding: JoinEncoding::default(),
                                                    is_static: false,
                                                },
                                            ),
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

        // 4. Perform the query and keep track of all the relevant chunks.
        let view_chunks = {
            let index_range = self
                .query
                .filtered_index_range
                .unwrap_or(ResolvedTimeRange::EVERYTHING);

            let query = RangeQuery::new(self.query.filtered_index, index_range)
                .keep_extra_timelines(true) // we want all the timelines we can get!
                .keep_extra_components(false);

            view_contents
                    .iter()
                    .map(|selected_column| match selected_column {
                        ColumnDescriptor::Control(_) | ColumnDescriptor::Time(_) => Vec::new(),

                        ColumnDescriptor::Component(column) => {
                        // NOTE: Keep in mind that the range APIs natively make sure that we will
                        // either get a bunch of relevant _static_ chunks, or a bunch of relevant
                        // _temporal_ chunks, but never both.
                        //
                        // TODO(cmc): Going through the cache is very useful in a Viewer context, but
                        // not so much in an SDK context. Make it configurable.
                        let results = self.engine.cache.range(
                            self.engine.store,
                            &query,
                            &column.entity_path,
                            [column.component_name],
                        );

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
                            .unwrap_or_default()
                        },
                    })
                    .collect()
        };

        QueryHandleState {
            view_contents,
            selected_contents,
            arrow_schema,
            view_chunks,
            cur_row: AtomicU64::new(0),
        }
    }

    /// The query used to instantiate this handle.
    pub fn query(&self) -> &QueryExpression2 {
        &self.query
    }

    /// Describes the columns that make up this view.
    ///
    /// See [`QueryExpression2::view_contents`].
    pub fn view_contents(&self) -> &[ColumnDescriptor] {
        &self.init().view_contents
    }

    /// Describes the columns that make up this selection.
    ///
    /// The extra `usize` is the index in [`Self::view_contents`] that this selection points to.
    ///
    /// See [`QueryExpression2::selection`].
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
        let all_unique_timestamps: HashSet<TimeInt> = self
            .init()
            .view_chunks
            .iter()
            .flat_map(|chunks| {
                chunks.iter().filter_map(|(_cursor, chunk)| {
                    chunk
                        .timelines()
                        .get(&self.query.filtered_index)
                        .map(|time_column| time_column.times())
                })
            })
            .flatten()
            .collect();

        all_unique_timestamps.len() as _
    }

    /// Returns the next row's worth of data.
    ///
    /// The returned vector of Arrow arrays strictly follows the schema specified by [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// Each cell in the result corresponds to the latest _locally_ known value at that particular point in
    /// the index, for each respective `ColumnDescriptor`.
    /// See [`QueryExpression2::sparse_fill_strategy`] to go beyond local resolution.
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
        struct StreamingJoinState<'a> {
            /// Which `Chunk` is this?
            chunk: &'a Chunk,

            /// How far are we into this `Chunk`?
            cursor: &'a AtomicU64,

            /// What's the index value at the current cursor?
            index_value: TimeInt,

            /// What's the `RowId` at the current cursor?
            row_id: RowId,
        }

        let state = self.init();

        let _cur_row = state.cur_row.fetch_add(1, Ordering::Relaxed);

        // First, we need to find, among all the chunks available for the current view contents,
        // what is their index value for the current row?
        //
        // NOTE: Non-component columns don't have a streaming state, hence the optional layer.
        let mut view_streaming_state: Vec<Option<StreamingJoinState<'_>>> =
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
                let cursor_value = cur_cursor.load(Ordering::Relaxed) as usize;

                // TODO(cmc): This can easily be optimized by looking ahead and breaking as soon as chunks
                // stop overlapping.

                let Some(cur_row_id) = cur_chunk.row_ids().nth(cursor_value) else {
                    continue;
                };

                let Some(cur_index_value) = cur_chunk
                    .timelines()
                    .get(&self.query.filtered_index)
                    .map_or(Some(TimeInt::STATIC), |time_column| {
                        time_column
                            .times_raw()
                            .get(cursor_value)
                            .copied()
                            .map(TimeInt::new_temporal)
                    })
                else {
                    continue;
                };

                if let Some(streaming_state) = streaming_state.as_mut() {
                    let StreamingJoinState {
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
                        *chunk = chunk;
                        *cursor = cursor;
                        *index_value = cur_index_value;
                        *row_id = cur_row_id;
                    }
                } else {
                    *streaming_state = Some(StreamingJoinState {
                        chunk: cur_chunk,
                        cursor: cur_cursor,
                        index_value: cur_index_value,
                        row_id: cur_row_id,
                    });
                };
            }
        }

        // What's the index value we're looking for at the current iteration?
        let cur_index_value = view_streaming_state
            .iter()
            .flatten()
            // NOTE: We're purposefully ignoring RowId-related semantics here: we just want to know
            // the value we're looking for on the "main" index (dedupe semantics).
            .min_by_key(|streaming_state| streaming_state.index_value)
            .map(|streaming_state| streaming_state.index_value)?;

        for streaming_state in &mut view_streaming_state {
            if streaming_state.as_ref().map(|s| s.index_value) != Some(cur_index_value) {
                *streaming_state = None;
            }
        }

        // The most recent chunk in the current iteration, according to RowId semantics.
        let cur_most_recent_row = view_streaming_state
            .iter()
            .flatten()
            .max_by_key(|streaming_state| streaming_state.row_id)?;

        // We are stitching a bunch of unrelated cells together in order to create the final row
        // that is being returned.
        //
        // For this reason, we can only guarantee that the index being explicitly queried for
        // (`QueryExpression2::filtered_index`) will match for all these cells.
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
            // Unless we are currently iterating over a static row, then we know for sure that the
            // timeline being used as `filtered_index` is A) present and B) has for value `cur_index_value`.
            if cur_index_value != TimeInt::STATIC {
                let slice = cur_most_recent_row
                    .chunk
                    .timelines()
                    .get(&self.query.filtered_index)
                    .map(|time_column| {
                        time_column.times_array().sliced(
                            cur_most_recent_row.cursor.load(Ordering::Relaxed) as usize,
                            1,
                        )
                    });

                debug_assert!(
                    slice.is_some(),
                    "Timeline must exist, otherwise the query engine would have never returned that chunk in the first place",
                );

                // NOTE: Cannot fail, just want to stay away from unwraps.
                if let Some(slice) = slice {
                    max_value_per_index.insert(self.query.filtered_index, (cur_index_value, slice));
                }
            }

            view_streaming_state
                .iter()
                .flatten()
                .flat_map(|streaming_state| {
                    streaming_state
                        .chunk
                        .timelines()
                        .values()
                        // NOTE: Already took care of that one above.
                        .filter(|time_column| *time_column.timeline() != self.query.filtered_index)
                        // NOTE: Cannot fail, just want to stay away from unwraps.
                        .filter_map(|time_column| {
                            let cursor = streaming_state.cursor.load(Ordering::Relaxed) as usize;
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
        }

        // NOTE: Non-component entries have no data to slice, hence the optional layer.
        //
        // TODO(cmc): no point in slicing arrays that are not selected.
        let view_sliced_arrays: Vec<Option<_>> = view_streaming_state
            .iter()
            .map(|streaming_state| {
                // NOTE: Reminder: the only reason the streaming state could be `None` here is
                // because this column does not have data for the current index value (i.e. `null`).
                streaming_state.as_ref().and_then(|streaming_state| {
                    let cursor = streaming_state.cursor.fetch_add(1, Ordering::Relaxed);

                    debug_assert!(
                        streaming_state.chunk.components().len() <= 1,
                        "cannot possibly get more than one component with this query"
                    );

                    let list_array = streaming_state
                        .chunk
                        .components()
                        .first_key_value()
                        .map(|(_, list_array)| list_array.sliced(cursor as usize, 1));

                    debug_assert!(
                        list_array.is_some(),
                        "This must exist or the chunk wouldn't have been sliced to start with."
                    );

                    // NOTE: This cannot possibly return None, see assert above.
                    list_array
                })
            })
            .collect();

        // TODO(cmc): It would likely be worth it to allocate all these possible
        // null-arrays ahead of time, and just return a pointer to those in the failure
        // case here.

        // TODO: explain the two-pass process.

        let mut arrays: Vec<Option<Box<dyn ArrowArray>>> =
            vec![None; state.selected_contents.len()];

        state.selected_contents.iter().enumerate().for_each(
            |(selected_idx, (view_idx, column))| match column {
                ColumnDescriptor::Control(_) | ColumnDescriptor::Time(_) => {}

                ColumnDescriptor::Component(descr) => {
                    let list_array = view_sliced_arrays.get(*view_idx).cloned();
                    let column_exists = list_array.is_some();

                    if !column_exists {
                        arrays[selected_idx] =
                            Some(arrow2::array::new_null_array(column.datatype(), 1));
                        return;
                    }

                    arrays[selected_idx] = if let Some(list_array) = list_array.flatten() {
                        Some(list_array)
                    } else {
                        // The streaming state itself is missing, i.e. there is no value for this
                        // column at the current index value.

                        match self.query.sparse_fill_strategy {
                            re_chunk_store::SparseFillStrategy::None => {
                                eprintln!("early exit -- missing without fill");
                                Some(arrow2::array::new_null_array(column.datatype(), 1))
                            }

                            // NOTE: While it would be very tempting to resolve the latest-at state
                            // of the entire view contents at `filtered_index_range.start - 1` once
                            // during queryhandle initialization, and then bootstrap off of that, that
                            // would effectively close the door to efficient pagination forever, since
                            // we'd have to iterate over all the pages to compute the right latest-at
                            // value at t+n (i.e. no random access possible).
                            // Therefore, it is better to simply do this the "dumb" way.
                            //
                            // TODO(cmc): Still, as always, this can be made faster and smarter at
                            // the cost of some extra complexity (e.g. caching the result across
                            // consecutive nulls etc). Later.
                            re_chunk_store::SparseFillStrategy::LatestAtGlobal => {
                                let query = re_chunk::LatestAtQuery::new(
                                    self.query.filtered_index,
                                    cur_index_value,
                                );

                                let results = self.engine.cache.latest_at(
                                    self.engine.store,
                                    &query,
                                    &descr.entity_path,
                                    [descr.component_name],
                                );

                                let array = results
                                    .components
                                    .get(&descr.component_name)
                                    .and_then(|unit| {
                                        // TODO: holy that's ugly.
                                        unit.timelines()
                                            .values()
                                            // TODO: cannot possibly beat this one
                                            .filter(|time_column| {
                                                *time_column.timeline() != self.query.filtered_index
                                            })
                                            // NOTE: Cannot fail, just want to stay away from unwraps.
                                            .flat_map(|time_column| {
                                                time_column
                                                    .times_raw()
                                                    .iter()
                                                    .copied()
                                                    .map(TimeInt::new_temporal)
                                                    .map(|time| {
                                                        (
                                                            *time_column.timeline(),
                                                            (
                                                                time,
                                                                time_column
                                                                    .times_array()
                                                                    .sliced(0, 1),
                                                            ),
                                                        )
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

                                        unit.components()
                                            .get(&descr.component_name)
                                            .map(|list_array| list_array.to_boxed())
                                    })
                                    .unwrap_or_else(|| {
                                        arrow2::array::new_null_array(column.datatype(), 1)
                                    });

                                Some(array)
                            }
                        }
                    };
                }
            },
        );

        state.selected_contents.iter().enumerate().for_each(
            |(selected_idx, (_view_idx, column))| match column {
                ColumnDescriptor::Control(_) => {
                    arrays[selected_idx] = Some(
                        cur_most_recent_row.chunk.row_ids_array().sliced(
                            cur_most_recent_row
                                .cursor
                                .load(Ordering::Relaxed)
                                // NOTE: We did the cursor increments while computing the final sliced arrays,
                                // so we need to go back one tick for this.
                                .saturating_sub(1) as usize,
                            1,
                        ),
                    );
                }

                ColumnDescriptor::Time(descr) => {
                    arrays[selected_idx] =
                        Some(max_value_per_index.remove(&descr.timeline).map_or_else(
                            || arrow2::array::new_null_array(column.datatype(), 1),
                            |(_time, time_sliced)| time_sliced,
                        ));
                }

                ColumnDescriptor::Component(_) => {}
            },
        );

        let arrays = arrays.into_iter().flatten().collect_vec();
        debug_assert_eq!(state.arrow_schema.fields.len(), arrays.len());

        Some(arrays)
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
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
    use re_chunk_store::{
        ChunkStore, ChunkStoreConfig, LatestAtQuery, RangeQuery, ResolvedTimeRange, TimeInt,
    };
    use re_log_types::{
        build_frame_nr,
        example_components::{MyColor, MyIndex, MyPoint},
        EntityPath, TimeType, Timeline,
    };
    use re_types_core::{ComponentName, Loggable as _};

    use crate::QueryCache;

    use super::*;

    // TODO(cmc): at least one basic test for every feature in `QueryExpression2`.
    // In no particular order:
    // * [x] filtered_index
    // * [ ] view_contents
    // * [ ] filtered_index_range
    // * [ ] filtered_index_values
    // * [ ] sampled_index_values
    // * [ ] filtered_point_of_view
    // * [ ] sparse_fill_strategy
    // * [ ] selection

    #[test]
    fn query_filtered_index() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store = create_nasty_store()?;
        eprintln!("{store}");
        let query_cache = QueryCache::new(&store);
        let query_engine = QueryEngine {
            store: &store,
            cache: &query_cache,
        };

        let timeline = Timeline::new_sequence("frame_nr");
        let mut query = QueryExpression2::new(timeline);
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        let dataframe =
            concatenate_record_batches(&query_handle.into_batch_iter().collect_vec()).unwrap();
        eprintln!("{dataframe}");

        Ok(())
    }

    /// Returns a very nasty [`ChunkStore`] with all kinds of partial updates, chunk overlaps,
    /// repeated timestamps, duplicated chunks, etc.
    fn create_nasty_store() -> anyhow::Result<ChunkStore> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let entity_path = EntityPath::from("this/that");

        let frame0 = TimeInt::new_temporal(0);
        let frame1 = TimeInt::new_temporal(1);
        let frame2 = TimeInt::new_temporal(2);
        let frame3 = TimeInt::new_temporal(3);
        let frame4 = TimeInt::new_temporal(4);
        let frame5 = TimeInt::new_temporal(5);
        let frame6 = TimeInt::new_temporal(6);
        let frame7 = TimeInt::new_temporal(7);
        let frame8 = TimeInt::new_temporal(8);

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

        let row_id1_1 = RowId::new();
        let row_id1_3 = RowId::new();
        let row_id1_5 = RowId::new();
        let row_id1_7_1 = RowId::new();
        let row_id1_7_2 = RowId::new();
        let row_id1_7_3 = RowId::new();
        let chunk1 = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                row_id1_1,
                [build_frame_nr(frame1)],
                [
                    (MyPoint::name(), Some(&points1 as _)),
                    (MyColor::name(), None),
                ],
            )
            .with_sparse_component_batches(
                row_id1_3,
                [build_frame_nr(frame3)],
                [
                    (MyPoint::name(), Some(&points3 as _)),
                    (MyColor::name(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id1_5,
                [build_frame_nr(frame5)],
                [
                    (MyPoint::name(), Some(&points5 as _)),
                    (MyColor::name(), None),
                ],
            )
            .with_sparse_component_batches(
                row_id1_7_1,
                [build_frame_nr(frame7)],
                [(MyPoint::name(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_2,
                [build_frame_nr(frame7)],
                [(MyPoint::name(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                row_id1_7_3,
                [build_frame_nr(frame7)],
                [(MyPoint::name(), Some(&points7_3 as _))],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        store.insert_chunk(&chunk1)?;
        store.insert_chunk(&chunk1)?; // x2 !

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

        Ok(store)
    }

    fn concatenate_record_batches(batches: &[RecordBatch]) -> Option<RecordBatch> {
        assert!(batches.iter().map(|batch| &batch.schema).all_equal());

        let first = batches.first()?;

        let mut arrays = Vec::new();
        for (i, field) in first.schema.fields.iter().enumerate() {
            let array = arrow2::compute::concatenate::concatenate(
                &batches
                    .iter()
                    .map(|batch| &*batch.data[i] as &dyn ArrowArray)
                    .collect_vec(),
            )
            .unwrap();
            arrays.push(array);
        }

        Some(RecordBatch {
            schema: first.schema.clone(),
            data: ArrowChunk::new(arrays),
        })
    }
}
