use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use arrow::array::{
    ArrayRef as ArrowArrayRef, BooleanArray as ArrowBooleanArray,
    PrimitiveArray as ArrowPrimitiveArray, RecordBatch as ArrowRecordBatch, RecordBatchOptions,
};
use arrow::buffer::ScalarBuffer as ArrowScalarBuffer;
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
    ChunkStore, ColumnDescriptor, ComponentColumnDescriptor, Index, IndexColumnDescriptor,
    IndexValue, QueryExpression, SparseFillStrategy,
};
use re_log::{debug_assert, debug_assert_eq, debug_panic};
use re_log_types::AbsoluteTimeRange;
use re_query::{QueryCache, StorageEngineLike};
use re_sorbet::{
    ChunkColumnDescriptors, ColumnSelector, RowIdColumnDescriptor, TimeColumnSelector,
};
use re_types_core::arrow_helpers::as_array_ref;
use re_types_core::{Loggable as _, SerializedComponentColumn, archetypes};

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
    ///
    /// The corresponding index value can be obtained using `unique_index_values[cur_row]`.
    ///
    /// `unique_index_values[cur_row]`: [`QueryHandleState::unique_index_values`]
    cur_row: AtomicU64,

    /// All unique index values that can possibly be returned by this query.
    ///
    /// Guaranteed ascendingly sorted and deduped.
    ///
    /// See also [`QueryHandleState::cur_row`].
    unique_index_values: Vec<IndexValue>,
}

impl<E: StorageEngineLike> QueryHandle<E> {
    pub(crate) fn new(engine: E, query: QueryExpression) -> Self {
        Self {
            engine,
            query,
            state: Default::default(),
        }
    }
}

impl<E: StorageEngineLike> QueryHandle<E> {
    /// Lazily initialize internal private state.
    ///
    /// It is important that query handles stay cheap to create.
    #[tracing::instrument(level = "debug", skip_all)]
    fn init(&self) -> &QueryHandleState {
        self.engine
            .with(|store, cache| self.state.get_or_init(|| self.init_(store, cache)))
    }

    // NOTE: This is split in its own method otherwise it completely breaks `rustfmt`.
    fn init_(&self, store: &ChunkStore, cache: &QueryCache) -> QueryHandleState {
        re_tracing::profile_scope!("init");

        // The timeline doesn't matter if we're running in static-only mode.
        let filtered_index = self
            .query
            .filtered_index
            .unwrap_or_else(|| TimelineName::new(""));

        // 1. Compute the schema for the query.
        let view_contents_schema = store.schema_for_query(&self.query);
        let view_contents = view_contents_schema.indices_and_components();

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
        let arrow_schema = ArrowSchemaRef::from(ArrowSchema::new_with_metadata(
            selected_contents
                .iter()
                .map(|(_, descr)| descr.to_arrow_field(re_sorbet::BatchType::Dataframe))
                .collect::<ArrowFields>(),
            Default::default(),
        ));

        // 4. Perform the query and keep track of all the relevant chunks.
        let query = {
            let index_range = if self.query.filtered_index.is_none() {
                AbsoluteTimeRange::EMPTY // static-only
            } else if let Some(using_index_values) = self.query.using_index_values.as_ref() {
                using_index_values
                    .first()
                    .and_then(|start| using_index_values.last().map(|end| (start, end)))
                    .map_or(AbsoluteTimeRange::EMPTY, |(start, end)| {
                        AbsoluteTimeRange::new(*start, *end)
                    })
            } else {
                self.query
                    .filtered_index_range
                    .unwrap_or(AbsoluteTimeRange::EVERYTHING)
            };

            RangeQuery::new(filtered_index, index_range)
                .keep_extra_timelines(true) // we want all the timelines we can get!
                .keep_extra_components(false)
        };
        let (view_pov_chunks_idx, mut view_chunks) =
            self.fetch_view_chunks(store, cache, &query, &view_contents);

        // 5. Collect all relevant clear chunks and update the view accordingly.
        //
        // We'll turn the clears into actual empty arrays of the expected component type.
        {
            re_tracing::profile_scope!("clear_chunks");

            let clear_chunks = self.fetch_clear_chunks(store, cache, &query, &view_contents);
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

                        (AtomicU64::new(0), chunk)
                    }));

                    // The chunks were sorted that way before, and it needs to stay that way after.
                    chunks.sort_by_key(|(_cursor, chunk)| {
                        // NOTE: The chunk has been densified already: its global time range is the same as
                        // the time range for the specific component of interest.
                        chunk
                            .timelines()
                            .get(&filtered_index)
                            .map(|time_column| time_column.time_range())
                            .map_or(TimeInt::STATIC, |time_range| time_range.min())
                    });
                }
            }
        }

        // 6. Collect all unique index values.
        //
        // Used to achieve ~O(log(n)) pagination.
        let unique_index_values = if self.query.filtered_index.is_none() {
            vec![TimeInt::STATIC]
        } else if let Some(using_index_values) = self.query.using_index_values.as_ref() {
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
                    chunks.iter().filter_map(|(_cursor, chunk)| {
                        chunk
                            .timelines()
                            .get(&filtered_index)
                            .map(|time_column| time_column.times())
                    })
                })
                .flatten()
                .collect();

            if let Some(filtered_index_values) = self.query.filtered_index_values.as_ref() {
                all_unique_index_values.retain(|time| filtered_index_values.contains(time));
            }

            all_unique_index_values
                .into_iter()
                .filter(|index_value| !index_value.is_static())
                .collect_vec()
        };

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

                        let results =
                            cache.latest_at(&query, &descr.entity_path, [descr.component]);

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
            cur_row: AtomicU64::new(0),
            unique_index_values,
        }
    }

    #[tracing::instrument(level = "info", skip_all)]
    #[expect(clippy::unused_self)]
    fn compute_user_selection(
        &self,
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
                        .find(|(_idx, view_descr)| {
                            *view_descr.timeline().name() == *selected_timeline
                        })
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

    fn fetch_view_chunks(
        &self,
        store: &ChunkStore,
        cache: &QueryCache,
        query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> (Option<usize>, Vec<Vec<(AtomicU64, Chunk)>>) {
        let mut view_pov_chunks_idx = self.query.filtered_is_not_null.as_ref().map(|_| usize::MAX);

        let view_chunks = view_contents
            .iter()
            .enumerate()
            .map(|(idx, selected_column)| match selected_column {
                ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => Vec::new(),

                ColumnDescriptor::Component(column) => {
                    let chunks = self
                        .fetch_chunks(store, cache, query, &column.entity_path, [column.component])
                        .unwrap_or_default();

                    if let Some(pov) = self.query.filtered_is_not_null.as_ref()
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
        &self,
        store: &ChunkStore,
        cache: &QueryCache,
        query: &RangeQuery,
        view_contents: &[ColumnDescriptor],
    ) -> IntMap<EntityPath, Vec<Chunk>> {
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
                let flat_chunks = self
                    .fetch_chunks(store, cache, query, entity_path, components)
                    .map(|chunks| {
                        chunks
                            .into_iter()
                            .map(|(_cursor, chunk)| chunk)
                            .collect_vec()
                    })
                    .unwrap_or_default();

                let recursive_chunks =
                    entity_path_ancestors(entity_path).flat_map(|ancestor_path| {
                        self.fetch_chunks(store, cache, query, &ancestor_path, components)
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

    fn fetch_chunks(
        &self,
        _store: &ChunkStore,
        cache: &QueryCache,
        query: &RangeQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> Option<Vec<(AtomicU64, Chunk)>> {
        // NOTE: Keep in mind that the range APIs natively make sure that we will
        // either get a bunch of relevant _static_ chunks, or a bunch of relevant
        // _temporal_ chunks, but never both.
        //
        // TODO(cmc): Going through the cache is very useful in a Viewer context, but
        // not so much in an SDK context. Make it configurable.
        let results = cache.range(query, entity_path, components);

        debug_assert!(
            results.components.len() <= 1,
            "cannot possibly get more than one component with this query"
        );

        results
            .components
            .into_iter()
            .next()
            .map(|(_component_descr, chunks)| {
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
                            if let Some(index) = self.query.filtered_index.as_ref() {
                                chunk.is_timeline_sorted(index)
                            } else {
                                chunk.is_sorted()
                            },
                            "the query cache should have already taken care of sorting (and densifying!) the chunk",
                        );

                        // TODO(cmc): That'd be more elegant, but right now there is no way to
                        // avoid allocations and copies when using Arrow's `ListArray`.
                        //
                        // let chunk = chunk.deduped_latest_on_index(&query.timeline);

                        (AtomicU64::default(), chunk)
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
    /// ## Concurrency
    ///
    /// Cursors are implemented using atomic variables, which means calling any of the `seek_*`
    /// while iteration is concurrently ongoing is memory-safe but logically undefined racy
    /// behavior. Be careful.
    ///
    /// ## Performance
    ///
    /// This requires going through every chunk once, and for each chunk running a binary search if
    /// the chunk's time range contains the `index_value`.
    ///
    /// I.e.: it's pretty cheap already.
    #[tracing::instrument(level = "trace", skip_all)]
    #[inline]
    pub fn seek_to_row(&self, row_idx: usize) {
        let state = self.init();

        let Some(index_value) = state.unique_index_values.get(row_idx) else {
            return;
        };

        state.cur_row.store(row_idx as _, Ordering::Relaxed);
        self.seek_to_index_value(*index_value);
    }

    /// Advance all internal cursors so that the next row yielded will correspond to `index_value`.
    ///
    /// If `index_value` isn't present in the dataset, this seeks to the first index value
    /// available past that point, if any.
    ///
    /// ## Concurrency
    ///
    /// Cursors are implemented using atomic variables, which means calling any of the `seek_*`
    /// while iteration is concurrently ongoing is memory-safe but logically undefined racy
    /// behavior. Be careful.
    ///
    /// ## Performance
    ///
    /// This requires going through every chunk once, and for each chunk running a binary search if
    /// the chunk's time range contains the `index_value`.
    ///
    /// I.e.: it's pretty cheap already.
    #[tracing::instrument(level = "debug", skip_all)]
    fn seek_to_index_value(&self, index_value: IndexValue) {
        re_tracing::profile_function!();

        let state = self.init();

        if index_value.is_static() {
            for chunks in &state.view_chunks {
                for (cursor, _chunk) in chunks {
                    cursor.store(0, Ordering::Relaxed);
                }
            }
            return;
        }

        for chunks in &state.view_chunks {
            for (cursor, chunk) in chunks {
                // NOTE: The chunk has been densified already: its global time range is the same as
                // the time range for the specific component of interest.
                let Some(time_column) = chunk.timelines().get(&state.filtered_index) else {
                    continue;
                };

                let time_range = time_column.time_range();

                let new_cursor = if index_value < time_range.min() {
                    0
                } else if index_value > time_range.max() {
                    chunk.num_rows() as u64 /* yes, one past the end -- not a mistake */
                } else {
                    time_column
                        .times_raw()
                        .partition_point(|&time| time < index_value.as_i64())
                        as u64
                };

                cursor.store(new_cursor, Ordering::Relaxed);
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
    pub fn next_row(&self) -> Option<Vec<ArrayRef>> {
        self.engine
            .with(|store, cache| self._next_row(store, cache))
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
        &self,
    ) -> impl std::future::Future<Output = Option<Vec<ArrayRef>>> + use<E>
    where
        E: 'static + Send + Clone,
    {
        let res: Option<Option<_>> = self
            .engine
            .try_with(|store, cache| self._next_row(store, cache));

        let engine = self.engine.clone();
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn _next_row(&self, store: &ChunkStore, cache: &QueryCache) -> Option<Vec<ArrowArrayRef>> {
        re_tracing::profile_function!();

        /// Temporary state used to resolve the streaming join for the current iteration.
        #[derive(Debug)]
        struct StreamingJoinStateEntry<'a> {
            /// Which `Chunk` is this?
            chunk: &'a Chunk,

            /// How far are we into this `Chunk`?
            cursor: u64,

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

        // Although that's a synchronous lock, we probably don't need to worry about it until
        // there is proof to the contrary: we are in a specific `QueryHandle` after all, there's
        // really no good reason to be contending here in the first place.
        let state = self.state.get_or_init(move || self.init_(store, cache));

        let row_idx = state.cur_row.fetch_add(1, Ordering::Relaxed);
        let cur_index_value = state.unique_index_values.get(row_idx as usize)?;

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

            'overlaps: for (cur_cursor, cur_chunk) in view_chunks {
                // TODO(cmc): This can easily be optimized by looking ahead and breaking as soon as chunks
                // stop overlapping.

                // NOTE: Too soon to increment the cursor, we cannot know yet which chunks will or
                // will not be part of the current row.
                let mut cur_cursor_value = cur_cursor.load(Ordering::Relaxed);

                let cur_index_times_empty: &[i64] = &[];
                let cur_index_times = cur_chunk
                    .timelines()
                    .get(&state.filtered_index)
                    .map_or(cur_index_times_empty, |time_column| time_column.times_raw());
                let cur_index_row_ids = cur_chunk.row_ids_slice();

                let (index_value, cur_row_id) = 'walk: loop {
                    let (Some(mut index_value), Some(mut cur_row_id)) = (
                        cur_index_times
                            .get(cur_cursor_value as usize)
                            .copied()
                            .map(TimeInt::new_temporal),
                        cur_index_row_ids.get(cur_cursor_value as usize).copied(),
                    ) else {
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
                                cur_cursor_value = cur_cursor.fetch_add(1, Ordering::Relaxed) + 1;
                            } else {
                                break;
                            }
                        }

                        break 'walk (index_value, cur_row_id);
                    }

                    if index_value > *cur_index_value {
                        continue 'overlaps;
                    }

                    cur_cursor_value = cur_cursor.fetch_add(1, Ordering::Relaxed) + 1;
                };

                debug_assert_eq!(index_value, *cur_index_value);

                if let Some(streaming_state) = streaming_state.as_mut() {
                    let StreamingJoinStateEntry {
                        chunk,
                        cursor,
                        row_id,
                    } = streaming_state;

                    if cur_row_id > *row_id {
                        *chunk = cur_chunk;
                        *cursor = cur_cursor_value;
                        *row_id = cur_row_id;
                    }
                } else {
                    *streaming_state = Some(StreamingJoinStateEntry {
                        chunk: cur_chunk,
                        cursor: cur_cursor_value,
                        row_id: cur_row_id,
                    });
                }
            }
        }

        let mut view_streaming_state = view_streaming_state
            .into_iter()
            .map(|streaming_state| streaming_state.map(StreamingJoinState::StreamingJoinState))
            .collect_vec();

        // Static always wins, no matter what.
        for (selected_idx, static_state) in state.selected_static_values.iter().enumerate() {
            if let static_state @ Some(_) =
                static_state.clone().map(StreamingJoinState::Retrofilled)
            {
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

                *streaming_state = static_state;
            }
        }

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

                    let results =
                        cache.latest_at(&query, &descr.entity_path.clone(), [descr.component]);

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
                        let component_desc = state.view_contents.get_index_or_component(view_idx).and_then(|col| if let ColumnDescriptor::Component(descr) = col {
                            if let Some(component_type) = descr.component_type  { component_type.sanity_check(); }
                            Some(re_types_core::ComponentDescriptor {
                                component_type: descr.component_type,
                                archetype: descr.archetype,
                                component: descr.component,
                            })
                        } else {
                            None
                        })?;
                        unit.components().get_array(component_desc.component).cloned()
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
                ColumnDescriptor::RowId(_) => state
                    .view_chunks
                    .first()
                    .and_then(|vec| vec.first()) // TODO(#9922): verify that using the row:ids from the first chunk always makes sense
                    .map(|(row_idx, chunk)| {
                        as_array_ref(
                            chunk
                                .row_ids_array()
                                .slice(row_idx.load(Ordering::Acquire) as _, 1),
                        )
                    })
                    .unwrap_or_else(|| arrow::array::new_null_array(&RowId::arrow_datatype(), 1)),

                ColumnDescriptor::Time(descr) => max_value_per_index
                    .get(descr.timeline().name())
                    .map_or_else(
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

    /// Calls [`Self::next_row`] and wraps the result in a [`ArrowRecordBatch`].
    ///
    /// Only use this if you absolutely need a [`ArrowRecordBatch`] as this adds a
    /// some overhead for schema validation.
    ///
    /// See [`Self::next_row`] for more information.
    #[inline]
    pub fn next_row_batch(&self) -> Option<ArrowRecordBatch> {
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
    pub async fn next_row_batch_async(&self) -> Option<ArrowRecordBatch>
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
    pub fn iter(&self) -> impl Iterator<Item = Vec<ArrowArrayRef>> + '_ {
        std::iter::from_fn(move || self.next_row())
    }

    /// Returns an iterator backed by [`Self::next_row`].
    #[expect(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(self) -> impl Iterator<Item = Vec<ArrowArrayRef>> {
        std::iter::from_fn(move || self.next_row())
    }

    /// Returns an iterator backed by [`Self::next_row_batch`].
    pub fn batch_iter(&self) -> impl Iterator<Item = ArrowRecordBatch> + '_ {
        std::iter::from_fn(move || self.next_row_batch())
    }

    /// Returns an iterator backed by [`Self::next_row_batch`].
    pub fn into_batch_iter(self) -> impl Iterator<Item = ArrowRecordBatch> {
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concat_batches(
            query_handle.schema(),
            &query_handle.batch_iter().collect_vec(),
        )?;
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

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concat_batches(
            query_handle.schema(),
            &query_handle.batch_iter().collect_vec(),
        )?;
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
                [0, 30, 60, 90]
                    .into_iter()
                    .map(TimeInt::new_temporal)
                    .chain(std::iter::once(TimeInt::STATIC))
                    .collect(),
            ),
            ..Default::default()
        };
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        assert_eq!(
            query_engine.query(query.clone()).into_iter().count() as u64,
            query_handle.num_rows()
        );
        let dataframe = concat_batches(
            query_handle.schema(),
            &query_handle.batch_iter().collect_vec(),
        )?;
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
                    [0, 15, 30, 30, 45, 60, 75, 90]
                        .into_iter()
                        .map(TimeInt::new_temporal)
                        .chain(std::iter::once(TimeInt::STATIC))
                        .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
            eprintln!("{}", format_record_batch(&dataframe.clone()));

            assert_snapshot!(DisplayRB(dataframe));
        }

        // sparse-filled
        {
            let query = QueryExpression {
                filtered_index,
                using_index_values: Some(
                    [0, 15, 30, 30, 45, 60, 75, 90]
                        .into_iter()
                        .map(TimeInt::new_temporal)
                        .chain(std::iter::once(TimeInt::STATIC))
                        .collect(),
                ),
                sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
            assert_eq!(
                query_engine.query(query.clone()).into_iter().count() as u64,
                query_handle.num_rows()
            );
            let dataframe = concat_batches(
                query_handle.schema(),
                &query_handle.batch_iter().collect_vec(),
            )?;
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

            let query_handle = query_engine.query(query.clone());
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
                    let got = concat_batches(
                        query_handle.schema(),
                        &query_handle.batch_iter().take(3).collect_vec(),
                    )?;

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

            let query_handle = query_engine.query(query.clone());
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
                    let got = concat_batches(
                        query_handle.schema(),
                        &query_handle.batch_iter().take(3).collect_vec(),
                    )?;

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
                    [0, 15, 30, 30, 45, 60, 75, 90]
                        .into_iter()
                        .map(TimeInt::new_temporal)
                        .chain(std::iter::once(TimeInt::STATIC))
                        .collect(),
                ),
                ..Default::default()
            };
            eprintln!("{query:#?}:");

            let query_handle = query_engine.query(query.clone());
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
                    let got = concat_batches(
                        query_handle.schema(),
                        &query_handle.batch_iter().take(3).collect_vec(),
                    )?;

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

            let query_handle = query_engine.query(query.clone());
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
                    let got = concat_batches(
                        query_handle.schema(),
                        &query_handle.batch_iter().take(3).collect_vec(),
                    )?;

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

        let query_handle = engine.query(query_expr);

        let dataframe = concat_batches(
            query_handle.schema(),
            &query_handle.batch_iter().collect_vec(),
        )?;
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
                self: std::pin::Pin<&mut Self>,
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
}
