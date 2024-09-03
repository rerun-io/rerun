use std::{collections::VecDeque, sync::OnceLock};

use ahash::HashMap;
use arrow2::{
    array::{Array as ArrowArray, ListArray as ArrowListArray},
    chunk::Chunk as ArrowChunk,
    datatypes::Schema as ArrowSchema,
};
use itertools::Itertools;

use re_chunk::{Chunk, LatestAtQuery, RangeQuery};
use re_chunk_store::{ColumnDescriptor, ComponentColumnDescriptor, RangeQueryExpression};

use crate::{QueryEngine, RecordBatch};

// ---

/// A handle to a range query, ready to be executed.
///
/// Cheaply created via [`QueryEngine::range`].
///
/// See [`RangeQueryHandle::next_page`].
//
// TODO(cmc): pagination support
// TODO(cmc): intra-timestamp decimation support
pub struct RangeQueryHandle<'a> {
    /// Handle to the [`QueryEngine`].
    pub(crate) engine: &'a QueryEngine<'a>,

    /// The original query expression used to instantiate this handle.
    pub(crate) query: RangeQueryExpression,

    /// The user-specified schema that describes any data returned through this handle, if any.
    pub(crate) user_columns: Option<Vec<ColumnDescriptor>>,

    /// Internal private state. Lazily computed.
    ///
    /// It is important that handles stay cheap to create.
    state: OnceLock<RangeQuerytHandleState>,
}

/// Internal private state. Lazily computed.
struct RangeQuerytHandleState {
    /// The final schema.
    columns: Vec<ColumnDescriptor>,

    /// All the [`Chunk`]s for the active point-of-view.
    ///
    /// These are already sorted and vertically sliced according to the query.
    pov_chunks: Option<VecDeque<Chunk>>,
}

impl<'a> RangeQueryHandle<'a> {
    pub(crate) fn new(
        engine: &'a QueryEngine<'a>,
        query: RangeQueryExpression,
        user_columns: Option<Vec<ColumnDescriptor>>,
    ) -> Self {
        Self {
            engine,
            query,
            user_columns,
            state: Default::default(),
        }
    }
}

impl RangeQueryHandle<'_> {
    /// Lazily initialize internal private state.
    ///
    /// It is important that handles stay cheap to create.
    fn init(&self) -> &RangeQuerytHandleState {
        self.state.get_or_init(|| {
            re_tracing::profile_scope!("init");

            let columns = {
                re_tracing::profile_scope!("compute schema");

                self.user_columns.clone().unwrap_or_else(|| {
                    self.engine
                        .store
                        .schema_for_query(&self.query.clone().into())
                })
            };

            let pov_chunks = {
                re_tracing::profile_scope!("gather pov timestamps");

                let query = RangeQuery::new(self.query.timeline, self.query.time_range)
                    .keep_extra_timelines(true) // we want all the timelines we can get!
                    .keep_extra_components(false);

                let results = self.engine.cache.range(
                    self.engine.store,
                    &query,
                    &self.query.pov.entity_path,
                    [self.query.pov.component_name],
                );

                results
                    .components
                    .into_iter()
                    .find_map(|(component_name, chunks)| {
                        (component_name == self.query.pov.component_name).then_some(chunks)
                    })
                    .map(Into::into)
            };

            RangeQuerytHandleState {
                columns,
                pov_chunks,
            }
        })
    }

    /// All results returned by this handle will strictly follow this schema.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    pub fn schema(&self) -> &[ColumnDescriptor] {
        &self.init().columns
    }

    /// Partially executes the range query until the next natural page of results.
    ///
    /// Returns a single [`RecordBatch`] containing as many rows as available in the page, or
    /// `None` if all the dataset has been returned.
    /// Each cell in the result corresponds to the latest known value at that particular point in time
    /// for each respective `ColumnDescriptor`.
    ///
    /// The schema of the returned [`RecordBatch`] is guaranteed to match the one returned by
    /// [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// "Natural pages" refers to pages of data that match 1:1 to the underlying storage.
    /// The size of each page cannot be known in advance, as it depends on unspecified
    /// implementation details.
    /// This is the most performant way to iterate over the dataset.
    ///
    /// ```ignore
    /// while let Some(batch) = query_handle.next_page() {
    ///     // …
    /// }
    pub fn next_page(&mut self) -> Option<RecordBatch> {
        re_tracing::profile_function!(format!("{:?}", self.query));

        _ = self.init();
        let pov_chunk = self.state.get_mut()?.pov_chunks.as_mut()?.pop_front()?;
        let pov_time_column = pov_chunk.timelines().get(&self.query.timeline)?;
        let columns = self.schema();

        // TODO(cmc): There are more efficient, albeit infinitely more complicated ways to do this.
        // Let's first implement all features (multi-PoV, pagination, timestamp streaming, etc) and
        // see if this ever becomes an issue before going down this road.
        //
        // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
        let list_arrays: HashMap<&ComponentColumnDescriptor, ArrowListArray<i32>> = {
            re_tracing::profile_scope!("queries");

            columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Component(descr) => Some(descr),
                    _ => None,
                })
                .filter_map(|descr| {
                    let arrays = pov_time_column
                        .times()
                        .map(|time| {
                            let query = LatestAtQuery::new(self.query.timeline, time);

                            let results = self.engine.cache.latest_at(
                                self.engine.store,
                                &query,
                                &descr.entity_path,
                                [descr.component_name],
                            );

                            results
                                .components
                                .get(&descr.component_name)
                                .and_then(|unit| {
                                    unit.component_batch_raw(&descr.component_name).clone()
                                })
                        })
                        .collect_vec();
                    let arrays = arrays
                        .iter()
                        .map(|array| array.as_ref().map(|array| &**array as &dyn ArrowArray))
                        .collect_vec();

                    let list_array =
                        re_chunk::util::arrays_to_list_array(descr.datatype.clone(), &arrays);

                    if cfg!(debug_assertions) {
                        #[allow(clippy::unwrap_used)] // want to crash in dev
                        Some((descr, list_array.unwrap()))
                    } else {
                        // NOTE: Technically cannot ever happen, but I'd rather that than an uwnrap.
                        list_array.map(|list_array| (descr, list_array))
                    }
                })
                .collect()
        };

        // NOTE: Keep in mind this must match the ordering specified by `Self::schema`.
        let packed_arrays = {
            re_tracing::profile_scope!("packing");

            columns
                .iter()
                .map(|descr| match descr {
                    ColumnDescriptor::Control(_descr) => pov_chunk.row_ids_array().to_boxed(),

                    ColumnDescriptor::Time(descr) => {
                        let time_column = pov_chunk.timelines().get(&descr.timeline).cloned();

                        if cfg!(debug_assertions) {
                            #[allow(clippy::unwrap_used)] // want to crash in dev
                            time_column.unwrap().times_array().to_boxed()
                        } else {
                            // NOTE: Technically cannot ever happen, but I'd rather that than an uwnrap.
                            time_column.map_or_else(
                                || {
                                    arrow2::array::new_null_array(
                                        descr.datatype.clone(),
                                        pov_chunk.num_rows(),
                                    )
                                },
                                |time_column| time_column.times_array().to_boxed(),
                            )
                        }
                    }

                    ColumnDescriptor::Component(descr) => list_arrays.get(descr).map_or_else(
                        || {
                            arrow2::array::new_null_array(
                                descr.datatype.clone(),
                                pov_time_column.num_rows(),
                            )
                        },
                        |list_array| list_array.to_boxed(),
                    ),
                })
                .collect_vec()
        };

        Some(RecordBatch {
            schema: ArrowSchema {
                fields: columns
                    .iter()
                    .map(ColumnDescriptor::to_arrow_field)
                    .collect(),
                metadata: Default::default(),
            },
            data: ArrowChunk::new(packed_arrays),
        })
    }
}

impl<'a> RangeQueryHandle<'a> {
    #[allow(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(mut self) -> impl Iterator<Item = RecordBatch> + 'a {
        std::iter::from_fn(move || self.next_page())
    }
}
