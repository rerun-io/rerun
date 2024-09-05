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
    /// The query expression used to instantiate this handle.
    pub fn query(&self) -> &LatestAtQueryExpression {
        &self.query
    }

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
        re_tracing::profile_function!(format!("{:?}", self.query));

        let columns = self.schema();

        let schema = ArrowSchema {
            fields: columns
                .iter()
                .map(ColumnDescriptor::to_arrow_field)
                .collect(),

            // TODO(#6889): properly some sorbet stuff we want to get in there at some point.
            metadata: Default::default(),
        };

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
                            || arrow2::array::new_null_array(descr.datatype.clone(), 1),
                            |time_column| time_column.times_array().to_boxed(),
                        ))
                    }

                    ColumnDescriptor::Component(descr) => Some(
                        all_units
                            .get(descr)
                            .and_then(|chunk| chunk.components().get(&descr.component_name))
                            .map_or_else(
                                || arrow2::array::new_null_array(descr.datatype.clone(), 1),
                                |list_array| list_array.to_boxed(),
                            ),
                    ),
                })
                .collect()
        };

        RecordBatch {
            schema,
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
