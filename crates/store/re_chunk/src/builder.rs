use std::collections::BTreeMap;

use arrow2::{
    array::{Array as ArrowArray, PrimitiveArray as ArrowPrimitiveArray},
    datatypes::DataType as ArrowDatatype,
};
use itertools::Itertools;

use nohash_hasher::IntMap;
use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline};
use re_types_core::{AsComponents, ComponentBatch, ComponentName};

use crate::{Chunk, ChunkId, ChunkResult, RowId, TimeColumn};

// ---

/// Helper to incrementally build a [`Chunk`].
///
/// Can be created using [`Chunk::builder`].
pub struct ChunkBuilder {
    id: ChunkId,
    entity_path: EntityPath,

    row_ids: Vec<RowId>,
    timelines: BTreeMap<Timeline, TimeColumnBuilder>,
    components: BTreeMap<ComponentName, Vec<Option<Box<dyn ArrowArray>>>>,
}

impl Chunk {
    /// Initializes a new [`ChunkBuilder`].
    #[inline]
    pub fn builder(entity_path: EntityPath) -> ChunkBuilder {
        ChunkBuilder::new(ChunkId::new(), entity_path)
    }

    /// Initializes a new [`ChunkBuilder`].
    ///
    /// The final [`Chunk`] will have the specified `id`.
    #[inline]
    pub fn builder_with_id(id: ChunkId, entity_path: EntityPath) -> ChunkBuilder {
        ChunkBuilder::new(id, entity_path)
    }
}

impl ChunkBuilder {
    /// Initializes a new [`ChunkBuilder`].
    ///
    /// See also [`Chunk::builder`].
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

    /// Add a row's worth of data using the given sparse component data.
    pub fn with_sparse_row(
        mut self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        components: impl IntoIterator<Item = (ComponentName, Option<Box<dyn ArrowArray>>)>,
    ) -> Self {
        let components = components.into_iter().collect_vec();

        // Align all columns by appending null values for rows where we don't have data.
        for (component_name, _) in &components {
            let arrays = self.components.entry(*component_name).or_default();
            arrays.extend(
                std::iter::repeat(None).take(self.row_ids.len().saturating_sub(arrays.len())),
            );
        }

        self.row_ids.push(row_id);

        for (timeline, time) in timepoint.into() {
            self.timelines
                .entry(timeline)
                .or_insert_with(|| TimeColumn::builder(timeline))
                .with_row(time);
        }

        for (component_name, array) in components {
            self.components
                .entry(component_name)
                .or_default()
                .push(array);
        }

        // Align all columns by appending null values for rows where we don't have data.
        for arrays in self.components.values_mut() {
            arrays.extend(
                std::iter::repeat(None).take(self.row_ids.len().saturating_sub(arrays.len())),
            );
        }

        self
    }

    /// Add a row's worth of data using the given component data.
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

    /// Add a row's worth of data by destructuring an archetype into component columns.
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

    /// Add a row's worth of data by serializing a single [`ComponentBatch`].
    #[inline]
    pub fn with_component_batch(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batch: &dyn ComponentBatch,
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

    /// Add a row's worth of data by serializing many [`ComponentBatch`]es.
    #[inline]
    pub fn with_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<Item = &'a dyn ComponentBatch>,
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

    /// Add a row's worth of data by serializing many sparse [`ComponentBatch`]es.
    #[inline]
    pub fn with_sparse_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<Item = (ComponentName, Option<&'a dyn ComponentBatch>)>,
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

    /// Builds and returns the final [`Chunk`].
    ///
    /// The arrow datatype of each individual column will be guessed by inspecting the data.
    ///
    /// If any component column turns out to be fully sparse (i.e. only null values), that column
    /// will be stripped out (how could we guess its datatype without any single value to inspect)?
    ///
    /// This is generally the desired behavior but, if you want to make sure to keep fully sparse
    /// columns (can be useful e.g. for testing purposes), see [`ChunkBuilder::build_with_datatypes`]
    /// instead.
    ///
    /// This returns an error if the chunk fails to `sanity_check`.
    #[inline]
    pub fn build(self) -> ChunkResult<Chunk> {
        re_tracing::profile_function!();
        let Self {
            id,
            entity_path,
            row_ids,
            timelines,
            components,
        } = self;

        let timelines = {
            re_tracing::profile_scope!("timelines");
            timelines
                .into_iter()
                .map(|(timeline, time_column)| (timeline, time_column.build()))
                .collect()
        };

        let components = {
            re_tracing::profile_scope!("components");
            components
                .into_iter()
                .filter_map(|(component_name, arrays)| {
                    let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();
                    crate::util::arrays_to_list_array_opt(&arrays)
                        .map(|list_array| (component_name, list_array))
                })
                .collect()
        };

        Chunk::from_native_row_ids(id, entity_path, None, &row_ids, timelines, components)
    }

    /// Builds and returns the final [`Chunk`].
    ///
    /// The arrow datatype of each individual column will be guessed by inspecting the data.
    ///
    /// If any component column turns out to be fully sparse (i.e. only null values), `datatypes`
    /// will be used as a fallback.
    ///
    /// If any component column turns out to be fully sparse (i.e. only null values) _and_ doesn't
    /// have an explicit datatype passed in, that column will be stripped out (how could we guess
    /// its datatype without any single value to inspect)?
    ///
    /// You should rarely want to keep fully sparse columns around outside of testing scenarios.
    /// See [`Self::build`].
    ///
    /// This returns an error if the chunk fails to `sanity_check`.
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
                .map(|(timeline, time_column)| (timeline, time_column.build()))
                .collect(),
            components
                .into_iter()
                .filter_map(|(component_name, arrays)| {
                    let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();

                    // If we know the datatype in advance, we're able to keep even fully sparse
                    // columns around.
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

// ---

/// Helper to incrementally build a [`TimeColumn`].
///
/// Can be created using [`TimeColumn::builder`].
pub struct TimeColumnBuilder {
    timeline: Timeline,

    times: Vec<i64>,
}

impl TimeColumn {
    /// Initializes a new [`TimeColumnBuilder`].
    #[inline]
    pub fn builder(timeline: Timeline) -> TimeColumnBuilder {
        TimeColumnBuilder::new(timeline)
    }
}

impl TimeColumnBuilder {
    /// Initializes a new [`TimeColumnBuilder`].
    ///
    /// See also [`TimeColumn::builder`].
    #[inline]
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            times: Vec::new(),
        }
    }

    /// Add a row's worth of time data using the given timestamp.
    #[inline]
    pub fn with_row(&mut self, time: TimeInt) -> &mut Self {
        let Self { timeline: _, times } = self;

        times.push(time.as_i64());

        self
    }

    /// Builds and returns the final [`TimeColumn`].
    #[inline]
    pub fn build(self) -> TimeColumn {
        let Self { timeline, times } = self;

        let times = ArrowPrimitiveArray::<i64>::from_vec(times).to(timeline.datatype());
        TimeColumn::new(None, timeline, times)
    }
}
