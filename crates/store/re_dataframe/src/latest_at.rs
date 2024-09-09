use std::sync::OnceLock;

use ahash::HashMap;
use arrow2::{
    array::Array as ArrowArray, chunk::Chunk as ArrowChunk, datatypes::Schema as ArrowSchema,
};
use itertools::Itertools;

use re_chunk::{LatestAtQuery, TimeInt, Timeline, UnitChunkShared};
use re_chunk_store::{ColumnDescriptor, ComponentColumnDescriptor, LatestAtQueryExpression};

use crate::{QueryEngine, RecordBatch};

// ---

/// A handle to a latest-at query, ready to be executed.
///
/// Cheaply created via [`QueryEngine::latest_at`].
///
/// See [`LatestAtQueryHandle::get`].
pub struct LatestAtQueryHandle<'a> {
    /// Handle to the [`QueryEngine`].
    engine: &'a QueryEngine<'a>,

    /// The original query expression used to instantiate this handle.
    query: LatestAtQueryExpression,

    /// The user-specified schema that describes any data returned through this handle, if any.
    user_columns: Option<Vec<ColumnDescriptor>>,

    /// Internal private state. Lazily computed.
    ///
    /// It is important that handles stay cheap to create.
    state: OnceLock<LatestAtQueryHandleState>,
}

/// Internal private state. Lazily computed.
struct LatestAtQueryHandleState {
    /// The final schema.
    columns: Vec<ColumnDescriptor>,
}

impl<'a> LatestAtQueryHandle<'a> {
    pub(crate) fn new(
        engine: &'a QueryEngine<'a>,
        query: LatestAtQueryExpression,
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

impl LatestAtQueryHandle<'_> {
    /// All results returned by this handle will strictly follow this schema.
    ///
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    pub fn schema(&self) -> &[ColumnDescriptor] {
        let state = self.state.get_or_init(|| {
            let columns = {
                re_tracing::profile_scope!("compute schema");

                self.user_columns
                    .clone()
                    .unwrap_or_else(|| {
                        self.engine
                            .store
                            .schema_for_query(&self.query.clone().into())
                    })
                    .into_iter()
                    // NOTE: We drop `RowId`, as it doesn't make any sense in a compound row like the
                    // one we are returning.
                    .filter(|descr| !matches!(descr, ColumnDescriptor::Control(_)))
                    .collect()
            };

            LatestAtQueryHandleState { columns }
        });

        &state.columns
    }

    /// Performs the latest-at query.
    ///
    /// Returns a single [`RecordBatch`] containing a single row, where each cell corresponds to
    /// the latest known value at that particular point in time for each respective `ColumnDescriptor`.
    ///
    /// The schema of the returned [`RecordBatch`] is guaranteed to match the one returned by
    /// [`Self::schema`].
    /// Columns that do not yield any data will still be present in the results, filled with null values.
    pub fn get(&self) -> RecordBatch {
        re_tracing::profile_function!(format!("{}", self.query));

        let columns = self.schema();

        let all_units: HashMap<&ComponentColumnDescriptor, UnitChunkShared> = {
            re_tracing::profile_scope!("queries");

            // TODO(cmc): Opportunities for parallelization, if it proves to be a net positive in practice.
            let query = LatestAtQuery::new(self.query.timeline, self.query.at);
            columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Component(descr) => {
                        let results = self.engine.cache.latest_at(
                            self.engine.store,
                            &query,
                            &descr.entity_path,
                            [descr.component_name],
                        );

                        results
                            .components
                            .get(&descr.component_name)
                            .cloned()
                            .map(|chunk| (descr, chunk))
                    }

                    _ => None,
                })
                .collect()
        };

        let mut max_time_per_timeline = HashMap::<Timeline, (TimeInt, UnitChunkShared)>::default();
        {
            re_tracing::profile_scope!("compound times");

            let timelines = columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Time(descr) => Some(descr.timeline),
                    _ => None,
                })
                .collect_vec();

            for unit in all_units.values() {
                for &timeline in &timelines {
                    if let Some((time, _)) = unit.index(&timeline) {
                        max_time_per_timeline
                            .entry(timeline)
                            .and_modify(|(cur_time, cur_unit)| {
                                if *cur_time < time {
                                    *cur_time = time;
                                    *cur_unit = unit.clone();
                                }
                            })
                            .or_insert_with(|| (time, unit.clone()));
                    }
                }
            }
        }

        // If the query didn't return anything at all, we just want a properly empty Recordbatch with
        // the right schema.
        let null_array_length = max_time_per_timeline.get(&self.query.timeline).is_some() as usize;

        // NOTE: Keep in mind this must match the ordering specified by `Self::schema`.
        let packed_arrays = {
            re_tracing::profile_scope!("packing");

            columns
                .iter()
                .filter_map(|descr| match descr {
                    ColumnDescriptor::Control(_) => {
                        if cfg!(debug_assertions) {
                            unreachable!("filtered out during schema computation");
                        } else {
                            // NOTE: Technically cannot ever happen, but I'd rather that than an uwnrap.
                            None
                        }
                    }

                    ColumnDescriptor::Time(descr) => {
                        let time_column = max_time_per_timeline
                            .remove(&descr.timeline)
                            .and_then(|(_, chunk)| chunk.timelines().get(&descr.timeline).cloned());

                        Some(time_column.map_or_else(
                            || {
                                arrow2::array::new_null_array(
                                    descr.datatype.clone(),
                                    null_array_length,
                                )
                            },
                            |time_column| time_column.times_array().to_boxed(),
                        ))
                    }

                    ColumnDescriptor::Component(descr) => Some(
                        all_units
                            .get(descr)
                            .and_then(|chunk| chunk.components().get(&descr.component_name))
                            .map_or_else(
                                || {
                                    arrow2::array::new_null_array(
                                        descr.datatype.clone(),
                                        null_array_length,
                                    )
                                },
                                |list_array| list_array.to_boxed(),
                            ),
                    ),
                })
                .collect_vec()
        };

        RecordBatch {
            schema: ArrowSchema {
                fields: columns
                    .iter()
                    .map(|descr| descr.to_arrow_field())
                    .collect_vec(),
                metadata: Default::default(),
            },
            data: ArrowChunk::new(packed_arrays),
        }
    }
}

impl<'a> LatestAtQueryHandle<'a> {
    #[allow(clippy::should_implement_trait)] // we need an anonymous closure, this won't work
    pub fn into_iter(self) -> impl Iterator<Item = RecordBatch> + 'a {
        let mut yielded = false;
        std::iter::from_fn(move || {
            if yielded {
                None
            } else {
                yielded = true;
                Some(self.get())
            }
        })
    }
}

// ---

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{ArrowArray, Chunk, EntityPath, RowId, TimeInt, TimePoint, Timeline};
    use re_chunk_store::{
        ChunkStore, ChunkStoreConfig, ColumnDescriptor, ComponentColumnDescriptor,
        LatestAtQueryExpression, TimeColumnDescriptor,
    };
    use re_log_types::{example_components::MyPoint, StoreId, StoreKind};
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

        let query = LatestAtQueryExpression {
            entity_path_filter: "/**".into(),
            timeline: Timeline::log_time(),
            at: TimeInt::MAX,
        };

        let entity_path: EntityPath = "/points".into();
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
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Radius>(
                entity_path.clone(),
            )),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Color>(entity_path)),
        ];

        let handle = engine.latest_at(&query, Some(columns.clone()));
        let batch = handle.get();

        // The output should be an empty recordbatch with the right schema and empty arrays.
        assert_eq!(0, batch.num_rows());
        assert!(
            itertools::izip!(handle.schema(), batch.schema.fields.iter())
                .all(|(descr, field)| descr.to_arrow_field() == *field)
        );
        assert!(itertools::izip!(handle.schema(), batch.data.iter())
            .all(|(descr, array)| descr.datatype() == array.data_type()));
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
                    [&[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)] as _],
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

        let query = LatestAtQueryExpression {
            entity_path_filter: "/**".into(),
            timeline: Timeline::log_time(),
            at: TimeInt::MAX,
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
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Radius>(
                entity_path.clone(),
            )),
            ColumnDescriptor::Component(ComponentColumnDescriptor::new::<Color>(entity_path)),
        ];

        let handle = engine.latest_at(&query, Some(columns.clone()));
        let batch = handle.get();

        assert_eq!(1, batch.num_rows());
        assert_eq!(
            chunk.components().get(&MyPoint::name()).unwrap().to_boxed(),
            itertools::izip!(batch.schema.fields.iter(), batch.data.iter())
                .find_map(
                    |(field, array)| (field.name == MyPoint::name().short_name())
                        .then_some(array.clone())
                )
                .unwrap()
        );
        assert!(
            itertools::izip!(handle.schema(), batch.schema.fields.iter())
                .all(|(descr, field)| descr.to_arrow_field() == *field)
        );
    }
}
