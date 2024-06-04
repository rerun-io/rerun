use std::collections::BTreeMap;

use arrow2::{
    array::{Array as ArrowArray, PrimitiveArray as ArrowPrimitiveArray},
    datatypes::DataType as ArrowDatatype,
};
use itertools::Itertools;

use nohash_hasher::IntMap;
use re_log_types::{EntityPath, RowId, TimeInt, TimePoint, Timeline};
use re_types_core::{AsComponents, ComponentName};

use crate::{Chunk, ChunkId, ChunkResult, ChunkTimeline};

// ---

// TODO: doc
pub struct ChunkBuilder {
    id: ChunkId,
    entity_path: EntityPath,

    row_ids: Vec<RowId>,
    timelines: BTreeMap<Timeline, ChunkTimelineBuilder>,
    components: BTreeMap<ComponentName, Vec<Option<Box<dyn ArrowArray>>>>,
}

impl Chunk {
    #[inline]
    pub fn builder(entity_path: EntityPath) -> ChunkBuilder {
        ChunkBuilder::new(ChunkId::new(), entity_path)
    }

    #[inline]
    pub fn builder_with_id(id: ChunkId, entity_path: EntityPath) -> ChunkBuilder {
        ChunkBuilder::new(id, entity_path)
    }
}

// TODO: no check during -- only one sanity check at the end
impl ChunkBuilder {
    #[inline]
    pub fn new(id: ChunkId, entity_path: EntityPath) -> Self {
        Self {
            id,
            entity_path,

            row_ids: Vec::new(),
            timelines: BTreeMap::new(),
            components: BTreeMap::new(),
        }
    }

    pub fn with_sparse_row(
        mut self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        components: impl IntoIterator<Item = (ComponentName, Option<Box<dyn ArrowArray>>)>,
    ) -> Self {
        let components = components.into_iter().collect_vec();

        // TODO: pre-equilibrium
        for (component_name, _) in &components {
            let arrays = self.components.entry(*component_name).or_default();
            arrays.extend(
                std::iter::repeat(None).take(self.row_ids.len().saturating_sub(arrays.len())),
            );
        }

        self.row_ids.push(row_id);

        // TODO: will fail at build time if they dont have the same timelines
        for (timeline, time) in timepoint.into() {
            self.timelines
                .entry(timeline)
                .or_insert_with(|| ChunkTimeline::builder(timeline))
                .with_row(time);
        }

        for (component_name, array) in components {
            self.components
                .entry(component_name)
                .or_default()
                .push(array);
        }

        // TODO: post-equilibrium
        for arrays in self.components.values_mut() {
            arrays.extend(
                std::iter::repeat(None).take(self.row_ids.len().saturating_sub(arrays.len())),
            );
        }

        self
    }

    #[inline]
    pub fn with_row(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        components: impl IntoIterator<Item = (ComponentName, Box<dyn ArrowArray>)>,
    ) -> Self {
        self.with_sparse_row(
            row_id,
            timepoint,
            components
                .into_iter()
                .map(|(component_name, array)| (component_name, Some(array))),
        )
    }

    // TODO: insert a single row from archetype data
    #[inline]
    pub fn with_archetype(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        as_components: &dyn AsComponents,
    ) -> Self {
        let batches = as_components.as_component_batches();
        self.with_component_batches(
            row_id,
            timepoint,
            batches.iter().map(|batch| batch.as_ref()),
        )
    }

    // TODO: insert a single row from component data
    #[inline]
    pub fn with_component_batch(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batch: &dyn re_types_core::ComponentBatch,
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            component_batch
                .to_arrow()
                .ok()
                .map(|array| (component_batch.name(), array)),
        )
    }

    // TODO: insert a single row from component data
    #[inline]
    pub fn with_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<Item = &'a dyn re_types_core::ComponentBatch>,
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            component_batches.into_iter().filter_map(|component_batch| {
                component_batch
                    .to_arrow()
                    .ok()
                    .map(|array| (component_batch.name(), array))
            }),
        )
    }

    // TODO: insert a single row from component data
    #[inline]
    pub fn with_sparse_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<
            Item = (ComponentName, Option<&'a dyn re_types_core::ComponentBatch>),
        >,
    ) -> Self {
        self.with_sparse_row(
            row_id,
            timepoint,
            component_batches
                .into_iter()
                .map(|(component_name, component_batch)| {
                    (
                        component_name,
                        component_batch.and_then(|batch| batch.to_arrow().ok()),
                    )
                }),
        )
    }

    #[inline]
    pub fn build(self) -> ChunkResult<Chunk> {
        let Self {
            id,
            entity_path,
            row_ids,
            timelines,
            components,
        } = self;

        Chunk::from_native_row_ids(
            id,
            entity_path,
            None,
            &row_ids,
            timelines
                .into_iter()
                .map(|(timeline, time_chunk)| (timeline, time_chunk.build()))
                .collect(),
            components
                .into_iter()
                .filter_map(|(component_name, arrays)| {
                    let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();
                    crate::util::arrays_to_list_array_opt(&arrays)
                        .map(|list_array| (component_name, list_array))
                })
                .collect(),
        )
    }

    // TODO: doc
    #[inline]
    pub fn build_with_datatypes(
        self,
        datatypes: &IntMap<ComponentName, ArrowDatatype>,
    ) -> ChunkResult<Chunk> {
        let Self {
            id,
            entity_path,
            row_ids,
            timelines,
            components,
        } = self;

        Chunk::from_native_row_ids(
            id,
            entity_path,
            None,
            &row_ids,
            timelines
                .into_iter()
                .map(|(timeline, time_chunk)| (timeline, time_chunk.build()))
                .collect(),
            components
                .into_iter()
                .filter_map(|(component_name, arrays)| {
                    let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();
                    // TODO: explain
                    if let Some(datatype) = datatypes.get(&component_name) {
                        crate::util::arrays_to_list_array(datatype.clone(), &arrays)
                            .map(|list_array| (component_name, list_array))
                    } else {
                        crate::util::arrays_to_list_array_opt(&arrays)
                            .map(|list_array| (component_name, list_array))
                    }
                })
                .collect(),
        )
    }
}

pub struct ChunkTimelineBuilder {
    timeline: Timeline,

    times: Vec<i64>,
}

impl ChunkTimeline {
    #[inline]
    pub fn builder(timeline: Timeline) -> ChunkTimelineBuilder {
        ChunkTimelineBuilder::new(timeline)
    }
}

impl ChunkTimelineBuilder {
    #[inline]
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            times: Vec::new(),
        }
    }

    #[inline]
    pub fn with_row(&mut self, time: TimeInt) -> &mut Self {
        let Self { timeline: _, times } = self;

        times.push(time.as_i64());

        self
    }

    #[inline]
    pub fn build(self) -> ChunkTimeline {
        let Self { timeline, times } = self;

        let times = ArrowPrimitiveArray::<i64>::from_vec(times).to(timeline.datatype());
        ChunkTimeline::new(None, timeline, times)
    }
}
