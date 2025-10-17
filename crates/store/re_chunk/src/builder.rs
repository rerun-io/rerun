use arrow::{array::ArrayRef, datatypes::DataType as ArrowDatatype};
use itertools::Itertools as _;
use nohash_hasher::IntMap;

use re_log_types::{EntityPath, NonMinI64, TimePoint, Timeline, TimelineName};
use re_types_core::{AsComponents, ComponentBatch, ComponentDescriptor, SerializedComponentBatch};

use crate::{Chunk, ChunkId, ChunkResult, RowId, TimeColumn, chunk::ChunkComponents};

// ---

/// Helper to incrementally build a [`Chunk`].
///
/// Can be created using [`Chunk::builder`].
pub struct ChunkBuilder {
    id: ChunkId,
    entity_path: EntityPath,

    row_ids: Vec<RowId>,
    timelines: IntMap<TimelineName, TimeColumnBuilder>,
    components: IntMap<ComponentDescriptor, Vec<Option<ArrayRef>>>,
}

impl Chunk {
    /// Initializes a new [`ChunkBuilder`].
    #[inline]
    pub fn builder(entity_path: impl Into<EntityPath>) -> ChunkBuilder {
        ChunkBuilder::new(ChunkId::new(), entity_path.into())
    }

    /// Initializes a new [`ChunkBuilder`].
    ///
    /// The final [`Chunk`] will have the specified `id`.
    #[inline]
    pub fn builder_with_id(id: ChunkId, entity_path: impl Into<EntityPath>) -> ChunkBuilder {
        ChunkBuilder::new(id, entity_path.into())
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
            timelines: IntMap::default(),
            components: IntMap::default(),
        }
    }

    /// Add a row's worth of data using the given sparse component data.
    pub fn with_sparse_row(
        mut self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        components: impl IntoIterator<Item = (ComponentDescriptor, Option<ArrayRef>)>,
    ) -> Self {
        let components = components.into_iter().collect_vec();

        // Align all columns by appending null values for rows where we don't have data.
        for (component_desc, _) in &components {
            let arrays = self.components.entry(component_desc.clone()).or_default();
            arrays.extend(std::iter::repeat_n(
                None,
                self.row_ids.len().saturating_sub(arrays.len()),
            ));
        }

        self.row_ids.push(row_id);

        for (timeline, cell) in timepoint.into() {
            self.timelines
                .entry(timeline)
                .or_insert_with(|| TimeColumn::builder(Timeline::new(timeline, cell.typ())))
                .with_row(cell.value);
        }

        for (component_descr, array) in components {
            self.components
                .entry(component_descr)
                .or_default()
                .push(array);
        }

        // Align all columns by appending null values for rows where we don't have data.
        for arrays in self.components.values_mut() {
            arrays.extend(std::iter::repeat_n(
                None,
                self.row_ids.len().saturating_sub(arrays.len()),
            ));
        }

        self
    }

    /// Add a row's worth of data using the given component data.
    #[inline]
    pub fn with_row(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        components: impl IntoIterator<Item = (ComponentDescriptor, ArrayRef)>,
    ) -> Self {
        self.with_sparse_row(
            row_id,
            timepoint,
            components
                .into_iter()
                .map(|(component_descr, array)| (component_descr, Some(array))),
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
        let batches = as_components.as_serialized_batches();
        self.with_serialized_batches(row_id, timepoint, batches)
    }

    /// Add the serialized value of a single component to the chunk.
    pub fn with_component<Component: re_types_core::Component>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_descr: re_types_core::ComponentDescriptor,
        value: &Component,
    ) -> re_types_core::SerializationResult<Self> {
        debug_assert_eq!(component_descr.component_type, Some(Component::name()));
        Ok(self.with_serialized_batches(
            row_id,
            timepoint,
            vec![re_types_core::SerializedComponentBatch {
                descriptor: component_descr,
                array: Component::to_arrow([std::borrow::Cow::Borrowed(value)])?,
            }],
        ))
    }

    /// Add a row's worth of data by serializing a single [`ComponentBatch`].
    #[inline]
    pub fn with_component_batch(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batch: (ComponentDescriptor, &dyn ComponentBatch),
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            component_batch
                .1
                .to_arrow()
                .ok()
                .map(|array| (component_batch.0, array)),
        )
    }

    /// Add a row's worth of data by serializing many [`ComponentBatch`]es.
    #[inline]
    pub fn with_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<Item = (ComponentDescriptor, &'a dyn ComponentBatch)>,
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            component_batches
                .into_iter()
                .filter_map(|(component_descr, component_batch)| {
                    component_batch
                        .to_arrow()
                        .ok()
                        .map(|array| (component_descr, array))
                }),
        )
    }

    /// Add a row's worth of data by serializing many sparse [`ComponentBatch`]es.
    #[inline]
    pub fn with_sparse_component_batches<'a>(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<
            Item = (ComponentDescriptor, Option<&'a dyn ComponentBatch>),
        >,
    ) -> Self {
        self.with_sparse_row(
            row_id,
            timepoint,
            component_batches
                .into_iter()
                .map(|(component_desc, component_batch)| {
                    (
                        component_desc,
                        component_batch.and_then(|batch| batch.to_arrow().ok()),
                    )
                }),
        )
    }

    /// Add a row's worth of data by serializing a single [`ComponentBatch`].
    #[inline]
    pub fn with_serialized_batch(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batch: SerializedComponentBatch,
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            [(component_batch.descriptor, component_batch.array)],
        )
    }

    /// Add a row's worth of data by serializing many [`ComponentBatch`]es.
    #[inline]
    pub fn with_serialized_batches(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<Item = SerializedComponentBatch>,
    ) -> Self {
        self.with_row(
            row_id,
            timepoint,
            component_batches
                .into_iter()
                .map(|component_batch| (component_batch.descriptor, component_batch.array)),
        )
    }

    /// Add a row's worth of data by serializing many sparse [`ComponentBatch`]es.
    #[inline]
    pub fn with_sparse_serialized_batches(
        self,
        row_id: RowId,
        timepoint: impl Into<TimePoint>,
        component_batches: impl IntoIterator<
            Item = (ComponentDescriptor, Option<SerializedComponentBatch>),
        >,
    ) -> Self {
        self.with_sparse_row(
            row_id,
            timepoint,
            component_batches
                .into_iter()
                .map(|(component_desc, component_batch)| {
                    (component_desc, component_batch.map(|batch| batch.array))
                }),
        )
    }

    /// Add a static row's worth of data using the given component data.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_row(row_id, TimePoint::default(), components)`.
    #[inline]
    pub fn with_static_row(
        self,
        row_id: RowId,
        components: impl IntoIterator<Item = (ComponentDescriptor, ArrayRef)>,
    ) -> Self {
        self.with_row(row_id, TimePoint::default(), components)
    }

    /// Add a static row's worth of data using the given sparse component data.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_sparse_row(row_id, TimePoint::default(), components)`.
    #[inline]
    pub fn with_static_sparse_row(
        self,
        row_id: RowId,
        components: impl IntoIterator<Item = (ComponentDescriptor, Option<ArrayRef>)>,
    ) -> Self {
        self.with_sparse_row(row_id, TimePoint::default(), components)
    }

    /// Add a static row's worth of data by destructuring an archetype into component columns.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_archetype(row_id, TimePoint::default(), as_components)`.
    #[inline]
    pub fn with_static_archetype(self, row_id: RowId, as_components: &dyn AsComponents) -> Self {
        self.with_archetype(row_id, TimePoint::default(), as_components)
    }

    /// Add a static row's worth of data by serializing a single [`ComponentBatch`].
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_component_batch(row_id, TimePoint::default(), component_batch)`.
    #[inline]
    pub fn with_static_component_batch(
        self,
        row_id: RowId,
        component_batch: (ComponentDescriptor, &dyn ComponentBatch),
    ) -> Self {
        self.with_component_batch(row_id, TimePoint::default(), component_batch)
    }

    /// Add a static row's worth of data by serializing many [`ComponentBatch`]es.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_component_batches(row_id, TimePoint::default(), component_batches)`.
    #[inline]
    pub fn with_static_component_batches<'a>(
        self,
        row_id: RowId,
        component_batches: impl IntoIterator<Item = (ComponentDescriptor, &'a dyn ComponentBatch)>,
    ) -> Self {
        self.with_component_batches(row_id, TimePoint::default(), component_batches)
    }

    /// Add a static row's worth of data by serializing many sparse [`ComponentBatch`]es.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_sparse_component_batches(row_id, TimePoint::default(), component_batches)`.
    #[inline]
    pub fn with_static_sparse_component_batches<'a>(
        self,
        row_id: RowId,
        component_batches: impl IntoIterator<
            Item = (ComponentDescriptor, Option<&'a dyn ComponentBatch>),
        >,
    ) -> Self {
        self.with_sparse_component_batches(row_id, TimePoint::default(), component_batches)
    }

    /// Add a static row's worth of data by serializing a single [`SerializedComponentBatch`].
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_serialized_batch(row_id, TimePoint::default(), component_batch)`.
    #[inline]
    pub fn with_static_serialized_batch(
        self,
        row_id: RowId,
        component_batch: SerializedComponentBatch,
    ) -> Self {
        self.with_serialized_batch(row_id, TimePoint::default(), component_batch)
    }

    /// Add a static row's worth of data by serializing many [`SerializedComponentBatch`]es.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_serialized_batches(row_id, TimePoint::default(), component_batches)`.
    #[inline]
    pub fn with_static_serialized_batches(
        self,
        row_id: RowId,
        component_batches: impl IntoIterator<Item = SerializedComponentBatch>,
    ) -> Self {
        self.with_serialized_batches(row_id, TimePoint::default(), component_batches)
    }

    /// Add a static row's worth of data by serializing many sparse [`SerializedComponentBatch`]es.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_sparse_serialized_batches(row_id, TimePoint::default(), component_batches)`.
    #[inline]
    pub fn with_static_sparse_serialized_batches(
        self,
        row_id: RowId,
        component_batches: impl IntoIterator<
            Item = (ComponentDescriptor, Option<SerializedComponentBatch>),
        >,
    ) -> Self {
        self.with_sparse_serialized_batches(row_id, TimePoint::default(), component_batches)
    }

    /// Add the static serialized value of a single component to the chunk.
    ///
    /// This is a convenience method that adds data with an empty [`TimePoint`], meaning
    /// the data will be considered static/timeless.
    ///
    /// Equivalent to calling `with_component(row_id, TimePoint::default(), component_descr, value)`.
    #[inline]
    pub fn with_static_component<Component: re_types_core::Component>(
        self,
        row_id: RowId,
        component_descr: re_types_core::ComponentDescriptor,
        value: &Component,
    ) -> re_types_core::SerializationResult<Self> {
        self.with_component(row_id, TimePoint::default(), component_descr, value)
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
            ChunkComponents(
                components
                    .into_iter()
                    .filter_map(|(component_desc, arrays)| {
                        let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();
                        re_arrow_util::arrays_to_list_array_opt(&arrays)
                            .map(|list_array| (component_desc, list_array))
                    })
                    .collect(),
            )
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
        datatypes: &IntMap<ComponentDescriptor, ArrowDatatype>,
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
            {
                ChunkComponents(
                    components
                        .into_iter()
                        .filter_map(|(component_desc, arrays)| {
                            let arrays = arrays.iter().map(|array| array.as_deref()).collect_vec();
                            // If we know the datatype in advance, we're able to keep even fully sparse
                            // columns around.
                            if let Some(datatype) = datatypes.get(&component_desc) {
                                re_arrow_util::arrays_to_list_array(datatype.clone(), &arrays)
                                    .map(|list_array| (component_desc, list_array))
                            } else {
                                re_arrow_util::arrays_to_list_array_opt(&arrays)
                                    .map(|list_array| (component_desc, list_array))
                            }
                        })
                        .collect(),
                )
            },
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
    pub fn with_row(&mut self, time: NonMinI64) -> &mut Self {
        self.times.push(time.into());
        self
    }

    /// Builds and returns the final [`TimeColumn`].
    #[inline]
    pub fn build(self) -> TimeColumn {
        let Self { timeline, times } = self;
        TimeColumn::new(None, timeline, times.into())
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::{TimePoint, example_components::MyPoint};
    use re_types_core::{AsComponents, Component, Loggable};

    use super::*;

    /// Simple test archetype for testing
    #[derive(Debug, Clone)]
    struct TestPoints {
        points: Vec<MyPoint>,
    }

    impl AsComponents for TestPoints {
        fn as_serialized_batches(&self) -> Vec<re_types_core::SerializedComponentBatch> {
            vec![re_types_core::SerializedComponentBatch {
                descriptor: ComponentDescriptor {
                    archetype: Some("test.TestPoints".into()),
                    component: "test.TestPoints:points".into(),
                    component_type: Some(<MyPoint as Component>::name()),
                },
                array: <MyPoint as Loggable>::to_arrow(self.points.iter()).unwrap(),
            }]
        }
    }

    #[test]
    fn test_static_chunk_utilities() {
        let entity_path: EntityPath = "test/entity".into();

        // Test with_static_row
        let row_id1 = RowId::new();
        let points_array =
            <MyPoint as Loggable>::to_arrow([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])
                .unwrap();
        let component_desc = ComponentDescriptor {
            archetype: Some("test.TestPoints".into()),
            component: "test.TestPoints:points".into(),
            component_type: Some(<MyPoint as Component>::name()),
        };

        let chunk = Chunk::builder(entity_path.clone())
            .with_static_row(row_id1, [(component_desc.clone(), points_array)])
            .build()
            .unwrap();

        assert!(chunk.is_static());
        assert_eq!(chunk.num_rows(), 1);
        assert_eq!(chunk.entity_path(), &entity_path);
        assert_eq!(chunk.timelines().len(), 0);

        // Test with_static_archetype
        let row_id2 = RowId::new();
        let test_points = TestPoints {
            points: vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)],
        };

        let chunk2 = Chunk::builder(entity_path.clone())
            .with_static_archetype(row_id2, &test_points)
            .build()
            .unwrap();

        assert!(chunk2.is_static());
        assert_eq!(chunk2.num_rows(), 1);

        // Test with_static_component
        let row_id3 = RowId::new();
        let point = MyPoint::new(100.0, 200.0);

        let chunk3 = Chunk::builder(entity_path.clone())
            .with_static_component(row_id3, component_desc.clone(), &point)
            .unwrap()
            .build()
            .unwrap();

        assert!(chunk3.is_static());
        assert_eq!(chunk3.num_rows(), 1);

        // Test multiple static rows
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();
        let points1 = <MyPoint as Loggable>::to_arrow([MyPoint::new(1.0, 1.0)]).unwrap();
        let points2 = <MyPoint as Loggable>::to_arrow([MyPoint::new(2.0, 2.0)]).unwrap();

        let chunk4 = Chunk::builder(entity_path.clone())
            .with_static_row(row_id4, [(component_desc.clone(), points1)])
            .with_static_row(row_id5, [(component_desc.clone(), points2)])
            .build()
            .unwrap();

        assert!(chunk4.is_static());
        assert_eq!(chunk4.num_rows(), 2);
    }

    #[test]
    fn test_mixed_static_and_timed_behavior() {
        // Adding both static and timed data creates a timed chunk (static rows get implicit timeline data)
        let entity_path: EntityPath = "test/entity".into();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let points_array1 = <MyPoint as Loggable>::to_arrow([MyPoint::new(1.0, 2.0)]).unwrap();
        let points_array2 = <MyPoint as Loggable>::to_arrow([MyPoint::new(3.0, 4.0)]).unwrap();
        let component_desc = ComponentDescriptor {
            archetype: Some("test.TestPoints".into()),
            component: "test.TestPoints:points".into(),
            component_type: Some(<MyPoint as Component>::name()),
        };

        let timepoint = TimePoint::from([(
            re_log_types::Timeline::log_time(),
            re_log_types::TimeInt::new_temporal(1000),
        )]);

        // First create a purely timed chunk to test non-static behavior
        let chunk = Chunk::builder(entity_path.clone())
            .with_row(
                row_id1,
                timepoint.clone(),
                [(component_desc.clone(), points_array1)],
            )
            .with_row(row_id2, timepoint, [(component_desc, points_array2)])
            .build()
            .unwrap();

        // Should not be static because it has timeline data
        assert!(!chunk.is_static());
        assert_eq!(chunk.num_rows(), 2);
        assert_eq!(chunk.timelines().len(), 1);

        // Compare with purely static chunk
        let points_array3 = <MyPoint as Loggable>::to_arrow([MyPoint::new(5.0, 6.0)]).unwrap();
        let component_desc2 = ComponentDescriptor {
            archetype: Some("test.TestPoints".into()),
            component: "test.TestPoints:points".into(),
            component_type: Some(<MyPoint as Component>::name()),
        };
        let static_chunk = Chunk::builder(entity_path)
            .with_static_row(RowId::new(), [(component_desc2, points_array3)])
            .build()
            .unwrap();

        assert!(static_chunk.is_static());
        assert_eq!(static_chunk.num_rows(), 1);
        assert_eq!(static_chunk.timelines().len(), 0);
    }

    #[test]
    fn test_static_sparse_utilities() {
        let entity_path: EntityPath = "test/entity".into();
        let row_id = RowId::new();
        let points_array = <MyPoint as Loggable>::to_arrow([MyPoint::new(1.0, 2.0)]).unwrap();
        let component_desc = ComponentDescriptor {
            archetype: Some("test.TestPoints".into()),
            component: "test.TestPoints:points".into(),
            component_type: Some(<MyPoint as Component>::name()),
        };

        // Test with_static_sparse_row with Some data
        let chunk = Chunk::builder(entity_path.clone())
            .with_static_sparse_row(
                row_id,
                [(component_desc.clone(), Some(points_array.clone()))],
            )
            .build()
            .unwrap();

        assert!(chunk.is_static());
        assert_eq!(chunk.num_rows(), 1);

        // Test with_static_sparse_row with None data (should be empty chunk)
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_static_sparse_row(row_id, [(component_desc, None)])
            .build()
            .unwrap();

        assert!(chunk2.is_static());
        assert_eq!(chunk2.num_rows(), 1);
        // Component should be filtered out due to being fully sparse
        assert_eq!(chunk2.components().len(), 0);
    }
}
