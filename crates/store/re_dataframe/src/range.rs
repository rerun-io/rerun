use std::sync::{atomic::AtomicU64, OnceLock};

use ahash::HashMap;
use arrow2::{
    array::{Array as ArrowArray, DictionaryArray as ArrowDictionaryArray},
    chunk::Chunk as ArrowChunk,
    datatypes::Schema as ArrowSchema,
    Either,
};
use itertools::Itertools;

use re_chunk::{Chunk, LatestAtQuery, RangeQuery, RowId, TimeInt};
use re_chunk_store::{
    ColumnDescriptor, ComponentColumnDescriptor, JoinEncoding, RangeQueryExpression,
};

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
    /// The columns that will be used to populate the results
    columns: Vec<ColumnDescriptor>,

    /// The derived arrow schema from the columns. All returned
    /// record batches will have this schema.
    ///
    /// This may include conversion to dictionary-encoded data.
    arrow_schema: ArrowSchema,

    /// All the [`Chunk`]s for the active point-of-view.
    ///
    /// These are already sorted and vertically sliced according to the query.
    pov_chunks: Option<Vec<Chunk>>,

    /// Tracks the current page index. Used for [`RangeQueryHandle::next_page`].
    //
    // NOTE: The state is behind a `OnceLock`, the atomic just make some things simpler down the road.
    cur_page: AtomicU64,
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

            let schema = ArrowSchema {
                fields: columns
                    .iter()
                    .map(|descr| descr.to_arrow_field())
                    .collect_vec(),
                metadata: Default::default(),
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
            };

            RangeQuerytHandleState {
                columns,
                arrow_schema: schema,
                pov_chunks,
                cur_page: AtomicU64::new(0),
            }
        })
    }

    /// The query used to instantiate this handle.
    pub fn query(&self) -> &RangeQueryExpression {
        &self.query
    }

    /// All results returned by this handle will strictly follow this schema.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    pub fn schema(&self) -> &[ColumnDescriptor] {
        &self.init().columns
    }

    /// Partially executes the range query until the next natural page of results.
    ///
    /// Returns a vector of [`RecordBatch`]es that in total contain as many rows as available in the next
    /// "natural page" of data from the pof component, or `None` if all the dataset has been returned.
    ///
    /// At best, this will be a single [`RecordBatch`] containing a "natural page" of data, following the chunk
    /// size of the pov-component. This will happen when all queried data either belongs to
    /// the same chunk, or is requested using [`JoinEncoding::DictionaryEncode`].
    ///
    /// However, in the case of mixed chunks without dictionary encoding, the engine will fall
    /// back to a row-by-row approach, which can be less efficient.
    ///
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
    /// ```
    pub fn next_page(&mut self) -> Option<Vec<RecordBatch>> {
        re_tracing::profile_function!(format!("next_page({})", self.query));

        let state = self.init();
        let cur_page = state.cur_page.load(std::sync::atomic::Ordering::Relaxed);

        // If the query didn't return anything at all, we just want a properly empty Recordbatch with
        // the right schema (but only for page 0, otherwise it's just nothingness).
        if cur_page == 0 && state.pov_chunks.is_none() {
            let columns = self.schema();
            _ = state
                .cur_page
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Some(vec![RecordBatch {
                schema: state.arrow_schema.clone(),
                data: ArrowChunk::new(
                    columns
                        .iter()
                        .map(|descr| arrow2::array::new_null_array(descr.datatype().clone(), 0))
                        .collect_vec(),
                ),
            }]);
        }

        let pov_chunk = state.pov_chunks.as_ref()?.get(cur_page as usize)?;
        _ = state
            .cur_page
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Some(self.dense_batch_at_pov(&self.query.pov, pov_chunk, &state.arrow_schema))
    }

    /// Partially executes the range query in order to return the specified range of rows.
    ///
    /// Returns a vector of [`RecordBatch`]es: as many as required to fill the specified range.
    ///
    /// The exact size of the [`RecordBatch`]es is an implementation detail.
    ///
    /// At best, each [`RecordBatch`] will be a "natural page" of data, following the chunk
    /// size of the pov-component. This will happen when all queried data either belongs to
    /// the same chunk, or is requested as a [`JoinEncoding::DictionaryEncode`] column.
    ///
    /// However, in the case of mixed chunks without dictionary encoding, the engine will fall
    /// back to a row-by-row approach, which can be less efficient.
    ///
    /// Each cell in the result corresponds to the latest known value at that particular point in time
    /// for each respective `ColumnDescriptor`.
    ///
    /// The schema of the returned [`RecordBatch`]es is guaranteed to match the one returned by
    /// [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    ///
    /// "Natural pages" refers to pages of data that match 1:1 to the underlying storage.
    /// The size of each page cannot be known in advance, as it depends on unspecified
    /// implementation details. This is the most performant way to iterate over the dataset.
    ///
    //
    // TODO(cmc): This could be turned into an actual lazy iterator at some point.
    pub fn get(&self, offset: u64, mut len: u64) -> Vec<RecordBatch> {
        re_tracing::profile_function!(format!("get({offset}, {len}, {})", self.query));

        let state = self.init();

        // If the query didn't return anything at all, we just want a properly empty Recordbatch with
        // the right schema (but only at index 0, otherwise it's just nothingness).
        if offset == 0 && (len == 0 || state.pov_chunks.is_none()) {
            let columns = self.schema();
            return vec![RecordBatch {
                schema: state.arrow_schema.clone(),
                data: ArrowChunk::new(
                    columns
                        .iter()
                        .map(|descr| arrow2::array::new_null_array(descr.datatype().clone(), 0))
                        .collect_vec(),
                ),
            }];
        }

        let mut results = Vec::new();

        let Some(pov_chunks) = state.pov_chunks.as_ref() else {
            return results;
        };
        let mut pov_chunks = pov_chunks.iter();

        let mut cur_offset = 0;
        let Some(mut cur_pov_chunk) = pov_chunks.next().cloned() else {
            return results;
        };

        // Fast-forward until the first relevant PoV chunk.
        //
        // TODO(cmc): should keep an extra sorted datastructure and use a binsearch instead.
        while (cur_offset + cur_pov_chunk.num_rows() as u64) < offset {
            cur_offset += cur_pov_chunk.num_rows() as u64;

            let Some(next_pov_chunk) = pov_chunks.next().cloned() else {
                return results;
            };
            cur_pov_chunk = next_pov_chunk;
        }

        // Fast-forward to until the first relevant row in the PoV chunk.
        let mut offset = if cur_offset < offset {
            offset.saturating_sub(cur_offset)
        } else {
            0
        };

        // Repeatedly compute dense ranges until we've returned `len` rows.
        while len > 0 {
            cur_pov_chunk = cur_pov_chunk.row_sliced(offset as _, len as _);
            results.extend(self.dense_batch_at_pov(
                &self.query.pov,
                &cur_pov_chunk,
                &state.arrow_schema,
            ));

            offset = 0; // always start at the first row after the first chunk
            len = len.saturating_sub(cur_pov_chunk.num_rows() as u64);

            let Some(next_pov_chunk) = pov_chunks.next().cloned() else {
                break;
            };
            cur_pov_chunk = next_pov_chunk;
        }

        results
    }

    /// How many chunks / natural pages of data will be returned?
    #[inline]
    pub fn num_chunks(&self) -> u64 {
        self.init()
            .pov_chunks
            .as_ref()
            .map_or(0, |pov_chunks| pov_chunks.len() as _)
    }

    /// How many rows of data will be returned?
    #[inline]
    pub fn num_rows(&self) -> u64 {
        self.init().pov_chunks.as_ref().map_or(0, |pov_chunks| {
            pov_chunks.iter().map(|chunk| chunk.num_rows() as u64).sum()
        })
    }

    fn dense_batch_at_pov(
        &self,
        pov: &ComponentColumnDescriptor,
        pov_chunk: &Chunk,
        schema: &ArrowSchema,
    ) -> Vec<RecordBatch> {
        let pov_time_column = pov_chunk.timelines().get(&self.query.timeline);
        let columns = self.schema();

        // TODO(cmc): There are more efficient, albeit infinitely more complicated ways to do this.
        // Let's first implement all features (multi-PoV, pagination, timestamp streaming, etc) and
        // see if this ever becomes an issue before going down this road.
        //
        // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
        let dict_arrays: HashMap<&ComponentColumnDescriptor, ArrowDictionaryArray<i32>> = {
            re_tracing::profile_scope!("dict queries");

            columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Component(descr) => match descr.join_encoding {
                        JoinEncoding::OverlappingSlice => None,
                        JoinEncoding::DictionaryEncode => Some(descr),
                    },
                    _ => None,
                })
                .filter_map(|descr| {
                    let arrays = pov_time_column
                        .map_or_else(
                            || Either::Left(std::iter::empty()),
                            |time_column| Either::Right(time_column.times()),
                        )
                        .chain(std::iter::repeat(TimeInt::STATIC))
                        .take(pov_chunk.num_rows())
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
                                    unit.component_batch_raw(&descr.component_name).clone().map(
                                        |array| {
                                            (
                                                unit.index(&query.timeline())
                                                    // NOTE: technically cannot happen, but better than unwrapping.
                                                    .unwrap_or((TimeInt::STATIC, RowId::ZERO)),
                                                array,
                                            )
                                        },
                                    )
                                })
                        })
                        .collect_vec();
                    let arrays = arrays
                        .iter()
                        .map(|array| {
                            array
                                .as_ref()
                                .map(|(index, array)| (index, &**array as &dyn ArrowArray))
                        })
                        .collect_vec();

                    let dict_array =
                        { re_chunk::util::arrays_to_dictionary(&descr.store_datatype, &arrays) };

                    if cfg!(debug_assertions) {
                        #[allow(clippy::unwrap_used)] // want to crash in dev
                        Some((descr, dict_array.unwrap()))
                    } else {
                        // NOTE: Technically cannot ever happen, but I'd rather that than an uwnrap.
                        dict_array.map(|dict_array| (descr, dict_array))
                    }
                })
                .collect()
        };

        let slice_arrays: HashMap<&ComponentColumnDescriptor, Vec<Option<Box<dyn ArrowArray>>>> = {
            re_tracing::profile_scope!("slice queries");

            columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Component(descr) => match descr.join_encoding {
                        JoinEncoding::OverlappingSlice => {
                            if descr != pov {
                                Some(descr)
                            } else {
                                None
                            }
                        }
                        JoinEncoding::DictionaryEncode => None,
                    },
                    _ => None,
                })
                .map(|descr| {
                    let arrays = pov_time_column
                        .map_or_else(
                            || Either::Left(std::iter::empty()),
                            |time_column| Either::Right(time_column.times()),
                        )
                        .chain(std::iter::repeat(TimeInt::STATIC))
                        .take(pov_chunk.num_rows())
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
                                    unit.clone()
                                        .into_chunk()
                                        .components()
                                        .get(&descr.component_name)
                                        .map(|arr| arr.to_boxed())
                                })
                        })
                        .collect_vec();

                    (descr, arrays)
                })
                .collect()
        };

        if slice_arrays.is_empty() {
            // NOTE: Keep in mind this must match the ordering specified by `Self::schema`.
            let packed_arrays = {
                re_tracing::profile_scope!("packing");

                columns
                    .iter()
                    .map(|descr| match descr {
                        ColumnDescriptor::Control(_descr) => pov_chunk.row_ids_array().to_boxed(),

                        ColumnDescriptor::Time(descr) => {
                            let time_column = pov_chunk.timelines().get(&descr.timeline).cloned();
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

                        ColumnDescriptor::Component(descr) => match descr.join_encoding {
                            JoinEncoding::OverlappingSlice => {
                                if descr == pov {
                                    pov_chunk
                                        .components()
                                        .get(&descr.component_name)
                                        .map_or_else(
                                            || {
                                                arrow2::array::new_null_array(
                                                    descr.returned_datatype(),
                                                    pov_chunk.num_rows(),
                                                )
                                            },
                                            |arr| arr.to_boxed(),
                                        )
                                } else {
                                    unreachable!()
                                }
                            }
                            JoinEncoding::DictionaryEncode => dict_arrays.get(descr).map_or_else(
                                || {
                                    arrow2::array::new_null_array(
                                        descr.returned_datatype(),
                                        pov_chunk.num_rows(),
                                    )
                                },
                                |dict_array| dict_array.to_boxed(),
                            ),
                        },
                    })
                    .collect_vec()
            };
            vec![RecordBatch {
                schema: schema.clone(),
                data: ArrowChunk::new(packed_arrays),
            }]
        } else {
            (0..pov_chunk.num_rows())
                .map(|row| {
                    // NOTE: Keep in mind this must match the ordering specified by `Self::schema`.
                    let packed_arrays = columns
                        .iter()
                        .map(|descr| match descr {
                            ColumnDescriptor::Control(_descr) => {
                                pov_chunk.row_ids_array().sliced(row, 1).to_boxed()
                            }

                            ColumnDescriptor::Time(descr) => {
                                let time_column = pov_chunk.timelines().get(&descr.timeline);
                                time_column.map_or_else(
                                    || arrow2::array::new_null_array(descr.datatype.clone(), 1),
                                    |time_column| {
                                        time_column.times_array().sliced(row, 1).to_boxed()
                                    },
                                )
                            }

                            ColumnDescriptor::Component(descr) => match descr.join_encoding {
                                JoinEncoding::OverlappingSlice => {
                                    if descr == pov {
                                        pov_chunk
                                            .components()
                                            .get(&descr.component_name)
                                            .map_or_else(
                                                || {
                                                    arrow2::array::new_null_array(
                                                        descr.returned_datatype(),
                                                        1,
                                                    )
                                                },
                                                |arr| arr.sliced(row, 1).to_boxed(),
                                            )
                                    } else {
                                        slice_arrays
                                            .get(descr)
                                            .and_then(|col| col.get(row).cloned())
                                            .flatten()
                                            .map_or_else(
                                                || {
                                                    arrow2::array::new_null_array(
                                                        descr.returned_datatype(),
                                                        1,
                                                    )
                                                },
                                                |arr| arr,
                                            )
                                    }
                                }

                                JoinEncoding::DictionaryEncode => {
                                    dict_arrays.get(descr).map_or_else(
                                        || {
                                            arrow2::array::new_null_array(
                                                descr.returned_datatype(),
                                                1,
                                            )
                                        },
                                        |dict_array| dict_array.sliced(row, 1).to_boxed(),
                                    )
                                }
                            },
                        })
                        .collect_vec();

                    RecordBatch {
                        schema: schema.clone(),
                        data: ArrowChunk::new(packed_arrays),
                    }
                })
                .collect()
        }
    }
}

impl<'a> RangeQueryHandle<'a> {
    #[allow(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(mut self) -> impl Iterator<Item = RecordBatch> + 'a {
        std::iter::from_fn(move || self.next_page()).flatten()
    }
}

// ---

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow2::array::DictionaryArray as ArrowDictionaryArray;

    use re_chunk::{ArrowArray, Chunk, EntityPath, RowId, TimePoint, Timeline};
    use re_chunk_store::{
        ChunkStore, ChunkStoreConfig, ColumnDescriptor, ComponentColumnDescriptor,
        RangeQueryExpression, TimeColumnDescriptor,
    };
    use re_log_types::{
        example_components::MyPoint, EntityPathFilter, ResolvedTimeRange, StoreId, StoreKind,
    };
    use re_query::Caches;
    use re_types::{
        components::{Color, Position3D, Radius},
        Loggable,
    };

    use crate::QueryEngine;

    #[test]
    fn empty_yields_empty() {
        let store = ChunkStore::new(
            StoreId::random(StoreKind::Recording),
            ChunkStoreConfig::default(),
        );
        let cache = Caches::new(&store);
        let engine = QueryEngine {
            store: &store,
            cache: &cache,
        };

        let entity_path: EntityPath = "/points".into();

        let query = RangeQueryExpression {
            entity_path_filter: EntityPathFilter::all(),
            timeline: Timeline::log_time(),
            time_range: ResolvedTimeRange::EVERYTHING,
            pov: ComponentColumnDescriptor::new::<Position3D>(entity_path.clone()),
        };

        let columns = vec![
            ColumnDescriptor::Time(TimeColumnDescriptor {
                timeline: Timeline::log_time(),
                datatype: Timeline::log_time().datatype(),
            }),
            ColumnDescriptor::Time(TimeColumnDescriptor {
                timeline: Timeline::log_tick(),
                datatype: Timeline::log_tick().datatype(),
            }),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Position3D>(
                entity_path.clone(),
            )),
            ColumnDescriptor::Component(
                ComponentColumnDescriptor::new::<Radius>(entity_path.clone())
                    .with_join_encoding(re_chunk_store::JoinEncoding::DictionaryEncode),
            ),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Color>(entity_path)),
        ];

        let mut handle = engine.range(&query, Some(columns.clone()));

        // Iterator API
        {
            let batches = handle.next_page().unwrap();
            // The output should be an empty recordbatch with the right schema and empty arrays.
            for batch in batches {
                assert_eq!(0, batch.num_rows());
                assert!(
                    itertools::izip!(handle.schema(), batch.schema.fields.iter())
                        .all(|(descr, field)| descr.to_arrow_field() == *field)
                );
                assert!(itertools::izip!(handle.schema(), batch.data.iter())
                    .all(|(descr, array)| &descr.datatype() == array.data_type()));
            }
            let batch = handle.next_page();
            assert!(batch.is_none());
        }

        // Paginated API
        {
            let batch = handle.get(0, 0).pop().unwrap();
            // The output should be an empty recordbatch with the right schema and empty arrays.
            assert_eq!(0, batch.num_rows());
            assert!(
                itertools::izip!(handle.schema(), batch.schema.fields.iter())
                    .all(|(descr, field)| descr.to_arrow_field() == *field)
            );
            assert!(itertools::izip!(handle.schema(), batch.data.iter())
                .all(|(descr, array)| &descr.datatype() == array.data_type()));

            let _batch = handle.get(0, 1).pop().unwrap();

            let batch = handle.get(1, 1).pop();
            assert!(batch.is_none());
        }
    }

    #[test]
    fn static_does_yield() {
        let mut store = ChunkStore::new(
            StoreId::random(StoreKind::Recording),
            ChunkStoreConfig::default(),
        );

        let entity_path: EntityPath = "/points".into();
        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batches(
                    RowId::new(),
                    TimePoint::default(),
                    [
                        &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)] as _,
                        &[Radius(3.0.into()), Radius(4.0.into())] as _,
                    ],
                )
                .build()
                .unwrap(),
        );
        _ = store.insert_chunk(&chunk);

        eprintln!("{store}");

        let cache = Caches::new(&store);
        let engine = QueryEngine {
            store: &store,
            cache: &cache,
        };

        let query = RangeQueryExpression {
            entity_path_filter: EntityPathFilter::all(),
            timeline: Timeline::log_time(),
            time_range: ResolvedTimeRange::EVERYTHING,
            pov: ComponentColumnDescriptor::new::<MyPoint>(entity_path.clone()),
        };

        let columns = vec![
            ColumnDescriptor::Time(TimeColumnDescriptor {
                timeline: Timeline::log_time(),
                datatype: Timeline::log_time().datatype(),
            }),
            ColumnDescriptor::Time(TimeColumnDescriptor {
                timeline: Timeline::log_tick(),
                datatype: Timeline::log_tick().datatype(),
            }),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<MyPoint>(
                entity_path.clone(),
            )),
            ColumnDescriptor::Component(
                ComponentColumnDescriptor::new::<Radius>(entity_path.clone())
                    .with_join_encoding(re_chunk_store::JoinEncoding::DictionaryEncode),
            ),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Color>(entity_path)),
        ];

        let mut handle = engine.range(&query, Some(columns.clone()));

        // Iterator API
        {
            let batches = handle.next_page().unwrap();
            let batch = batches.first().unwrap();

            assert_eq!(1, batch.num_rows());

            // MyPoint should be a ListArray
            assert_eq!(
                chunk.components().get(&MyPoint::name()).unwrap().to_boxed(),
                itertools::izip!(batch.schema.fields.iter(), batch.data.iter())
                    .find_map(|(field, array)| {
                        (field.name == MyPoint::name().short_name()).then_some(array.clone())
                    })
                    .unwrap()
            );

            // Radius should be a DictionaryArray
            assert_eq!(
                chunk.components().get(&Radius::name()).unwrap().to_boxed(),
                itertools::izip!(batch.schema.fields.iter(), batch.data.iter())
                    .find_map(|(field, array)| {
                        (field.name == Radius::name().short_name()).then_some(array.clone())
                    })
                    .unwrap()
                    .as_any()
                    .downcast_ref::<ArrowDictionaryArray<i32>>()
                    .unwrap()
                    .values()
                    .clone()
            );

            assert!(
                itertools::izip!(handle.schema(), batch.schema.fields.iter())
                    .all(|(descr, field)| descr.to_arrow_field() == *field)
            );

            let batch = handle.next_page();
            assert!(batch.is_none());
        }

        // Paginated API
        {
            let batch = handle.get(0, 1).pop().unwrap();
            // The output should be an empty recordbatch with the right schema and empty arrays.
            assert_eq!(1, batch.num_rows());

            // MyPoint should be a ListArray
            assert_eq!(
                chunk.components().get(&MyPoint::name()).unwrap().to_boxed(),
                itertools::izip!(batch.schema.fields.iter(), batch.data.iter())
                    .find_map(|(field, array)| {
                        (field.name == MyPoint::name().short_name()).then_some(array.clone())
                    })
                    .unwrap()
            );

            // Radius should be a DictionaryArray
            assert_eq!(
                chunk.components().get(&Radius::name()).unwrap().to_boxed(),
                itertools::izip!(batch.schema.fields.iter(), batch.data.iter())
                    .find_map(|(field, array)| {
                        (field.name == Radius::name().short_name()).then_some(array.clone())
                    })
                    .unwrap()
                    .as_any()
                    .downcast_ref::<ArrowDictionaryArray<i32>>()
                    .unwrap()
                    .values()
                    .clone()
            );

            assert!(
                itertools::izip!(handle.schema(), batch.schema.fields.iter())
                    .all(|(descr, field)| descr.to_arrow_field() == *field)
            );

            // TODO(jleibs): Out-of-bounds behavior isn't well defined here.
            // Should this always include an empty record-batch, or should
            // it be an error?
            assert!(handle.get(1, 1).is_empty());
        }
    }
}
