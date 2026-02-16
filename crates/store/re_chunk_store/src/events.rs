use std::sync::Arc;

use re_chunk::Chunk;
use re_log_encoding::RrdManifest;
use re_log_types::StoreId;

use crate::{ChunkDirectLineageReport, ChunkStoreGeneration};

#[expect(unused_imports, clippy::unused_trait_names)] // used in docstrings
use crate::{ChunkId, ChunkStore, ChunkStoreSubscriber, RowId};

// ---

/// Per-component information for chunks.
///
/// Created from either a physical chunk or virtual manifest metadata.
#[derive(Clone)]
pub struct ChunkComponentMeta {
    pub descriptor: re_sdk_types::ComponentDescriptor,

    /// The component list's inner data type.
    ///
    /// `None` if unknown.
    pub inner_arrow_datatype: Option<arrow::datatypes::DataType>,

    /// True if there's actually any data logged for this component.
    ///
    /// For virtual this means `row_count > 0`.
    pub has_data: bool,

    /// Whether this component only has static data.
    pub is_static_only: bool,
}

/// Chunk meta originating from either a virtual or physical chunk.
///
/// Useful for chunk store subscribers that do the same logic
/// for physical and virtual additions.
#[derive(Clone)]
pub struct ChunkMeta {
    pub entity_path: re_chunk::EntityPath,
    pub components: Vec<ChunkComponentMeta>,
}

/// The atomic unit of change in the Rerun [`ChunkStore`].
///
/// A [`ChunkStoreEvent`] describes the changes caused by the addition or deletion of a
/// [`Chunk`] in the store.
///
/// Methods that mutate the [`ChunkStore`], such as [`ChunkStore::insert_chunk`] and [`ChunkStore::gc`],
/// return [`ChunkStoreEvent`]s that describe the changes.
/// You can also register your own [`ChunkStoreSubscriber`] in order to be notified of changes as soon as they
/// happen.
///
/// Refer to field-level documentation for more details and check out [`ChunkStoreDiff`] for a precise
/// definition of what an event involves.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkStoreEvent {
    /// Which [`ChunkStore`] sent this event?
    pub store_id: StoreId,

    /// What was the store's generation when it sent that event?
    pub store_generation: ChunkStoreGeneration,

    /// Monotonically increasing ID of the event.
    ///
    /// This is on a per-store basis.
    ///
    /// When handling a [`ChunkStoreEvent`], if this is the first time you process this [`StoreId`] and
    /// the associated `event_id` is not `1`, it means you registered late and missed some updates.
    pub event_id: u64,

    /// What actually changed?
    ///
    /// Refer to [`ChunkStoreDiff`] for more information.
    pub diff: ChunkStoreDiff,
}

impl std::ops::Deref for ChunkStoreEvent {
    type Target = ChunkStoreDiff;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.diff
    }
}

/// Describes an atomic change in the Rerun [`ChunkStore`]:
/// * a physical chunk has been added or deleted.
/// * a virtual manifest has been added.
///
/// From a query model standpoint, the [`ChunkStore`] _always_ operates one chunk at a time:
/// - The contents of a chunk (i.e. its columns) are immutable past insertion, by virtue of
///   [`ChunkId`]s being unique and non-reusable.
/// - Similarly, garbage collection always removes _all the data_ associated with a chunk in one go:
///   there cannot be orphaned columns. When a chunk is gone, all data associated with it is gone too.
///
/// Refer to field-level documentation for more information.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkStoreDiff {
    /// When a new physical chunk has been appended.
    Addition(ChunkStoreDiffAddition),

    /// When a new rrd manifest has been appended.
    VirtualAddition(ChunkStoreDiffVirtualAddition),

    /// When a physical chunk has been evicted.
    Deletion(ChunkStoreDiffDeletion),
}

impl From<ChunkStoreDiffAddition> for ChunkStoreDiff {
    fn from(value: ChunkStoreDiffAddition) -> Self {
        Self::Addition(value)
    }
}

impl From<ChunkStoreDiffVirtualAddition> for ChunkStoreDiff {
    fn from(value: ChunkStoreDiffVirtualAddition) -> Self {
        Self::VirtualAddition(value)
    }
}

impl From<ChunkStoreDiffDeletion> for ChunkStoreDiff {
    fn from(value: ChunkStoreDiffDeletion) -> Self {
        Self::Deletion(value)
    }
}

impl ChunkStoreDiff {
    pub fn addition(
        chunk_before_processing: Arc<Chunk>,
        chunk_after_processing: Arc<Chunk>,
        direct_lineage: ChunkDirectLineageReport,
    ) -> Self {
        Self::Addition(ChunkStoreDiffAddition {
            chunk_before_processing,
            chunk_after_processing,
            direct_lineage,
        })
    }

    pub fn virtual_addition(rrd_manifest: Arc<RrdManifest>) -> Self {
        Self::VirtualAddition(ChunkStoreDiffVirtualAddition { rrd_manifest })
    }

    pub fn deletion(chunk: Arc<Chunk>) -> Self {
        Self::Deletion(ChunkStoreDiffDeletion { chunk })
    }

    pub fn is_addition(&self) -> bool {
        matches!(self, Self::Addition(_))
    }

    pub fn is_virtual_addition(&self) -> bool {
        matches!(self, Self::VirtualAddition(_))
    }

    pub fn is_deletion(&self) -> bool {
        matches!(self, Self::Deletion(_))
    }

    pub fn into_addition(self) -> Option<ChunkStoreDiffAddition> {
        match self {
            Self::Addition(addition) => Some(addition),
            _ => None,
        }
    }

    pub fn into_virtual_addition(self) -> Option<ChunkStoreDiffVirtualAddition> {
        match self {
            Self::VirtualAddition(addition) => Some(addition),
            _ => None,
        }
    }

    pub fn into_deletion(self) -> Option<ChunkStoreDiffDeletion> {
        match self {
            Self::Deletion(deletion) => Some(deletion),
            _ => None,
        }
    }

    pub fn to_addition(&self) -> Option<&ChunkStoreDiffAddition> {
        match self {
            Self::Addition(addition) => Some(addition),
            _ => None,
        }
    }

    pub fn to_virtual_addition(&self) -> Option<&ChunkStoreDiffVirtualAddition> {
        match self {
            Self::VirtualAddition(addition) => Some(addition),
            _ => None,
        }
    }

    pub fn to_deletion(&self) -> Option<&ChunkStoreDiffDeletion> {
        match self {
            Self::Deletion(deletion) => Some(deletion),
            _ => None,
        }
    }

    /// `-1` for physical deletions, `+1` for physical additions. 0 otherwise.
    #[inline]
    pub fn delta(&self) -> i64 {
        match self {
            Self::Addition(_) => 1,
            Self::VirtualAddition(_) => 0,
            Self::Deletion(_) => -1,
        }
    }

    /// This always returns a chunk that only contains never-seen-before data.
    ///
    /// For a physical addition:
    /// * In case of a compaction event, this corresponds to the original chunk before compaction, which
    ///   only contains the newly added data.
    /// * In case of a split event, this corresponds to the individual splits, so the original data does not
    ///   get accounted for more than once.
    ///
    /// For a physical deletion, it returns the deleted chunk as-is.
    /// For a virtual addition, it returns `None`.
    pub fn delta_chunk(&self) -> Option<&Arc<Chunk>> {
        match self {
            Self::Addition(addition) => Some(addition.delta_chunk()),
            Self::VirtualAddition(_) => None,
            Self::Deletion(deletion) => Some(&deletion.chunk),
        }
    }
}

#[derive(Clone)]
pub struct ChunkStoreDiffAddition {
    /// The chunk that was added, *unaltered*.
    ///
    /// This is the chunk exactly as it was passed to the insertion method, before any kind of processing
    /// happened to it (compaction, splitting, etc).
    /// To access the compacted/split data, refer to [`ChunkStoreDiffAddition::chunk_after_processing`] instead.
    ///
    /// ## Relationship to [`ChunkStoreDiffAddition::direct_lineage`]
    ///
    /// If the lineage is…:
    /// * `SplitFrom`: then this is the original chunk, before splitting.
    /// * `CompactedFrom`: then this is the original chunk, before compaction.
    /// * anything else: then is the original chunk.
    ///
    /// When trying to count things, use [`ChunkStoreDiffAddition::delta_chunk`], which always returns
    /// just the chunk that contains unique data.
    //
    // NOTE: We purposefully use an `Arc` instead of a `ChunkId` here because we want to make sure that all
    // downstream subscribers get a chance to inspect the data in the chunk before it gets permanently
    // deallocated.
    pub chunk_before_processing: Arc<Chunk>,

    /// The chunk that was added, post-processing (splitting, compaction, etc).
    ///
    /// This is the chunk exactly as it was when it was finally indexed by the store, after all kinds
    /// of processing happened to it (compaction, splitting, etc).
    /// To access the unprocessed data, refer to [`ChunkStoreDiffAddition::chunk_before_processing`] instead.
    ///
    /// ## Relationship to [`ChunkStoreDiffAddition::direct_lineage`]
    ///
    /// If the lineage is…:
    /// * `SplitFrom`: then this is one of split siblings.
    /// * `CompactedFrom`: then this is the compacted chunk.
    /// * anything else: then this is the original chunk, same as [`ChunkStoreDiffAddition::chunk_before_processing`].
    ///
    /// When trying to count things, use [`ChunkStoreDiffAddition::delta_chunk`], which always returns
    /// just the chunk that contains unique data.
    //
    // NOTE: We purposefully use an `Arc` instead of a `ChunkId` here because we want to make sure that all
    // downstream subscribers get a chance to inspect the data in the chunk before it gets permanently
    // deallocated.
    pub chunk_after_processing: Arc<Chunk>,

    /// The direct lineage of [`ChunkStoreDiffAddition::chunk_after_processing`].
    ///
    /// This can be used to keep track of compactions and split-offs.
    ///
    /// This is not necessarily a compaction or split-off: the original root-level chunk might have
    /// been inserted as-is.
    pub direct_lineage: ChunkDirectLineageReport,
}

impl std::fmt::Debug for ChunkStoreDiffAddition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            chunk_before_processing,
            chunk_after_processing,
            direct_lineage,
        } = self;
        f.debug_struct("ChunkStoreDiffAddition")
            .field("chunk_before_processing", &chunk_before_processing.id())
            .field("chunk_after_processing", &chunk_after_processing.id())
            .field("direct_lineage", direct_lineage)
            .finish()
    }
}

impl PartialEq for ChunkStoreDiffAddition {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            chunk_before_processing,
            chunk_after_processing,
            direct_lineage,
        } = self;
        chunk_before_processing.id() == other.chunk_before_processing.id()
            && chunk_after_processing.id() == other.chunk_after_processing.id()
            && *direct_lineage == other.direct_lineage
    }
}

impl ChunkStoreDiffAddition {
    /// This always returns a chunk that only contains never-seen-before data.
    ///
    /// For a compaction event, this corresponds to the original chunk before compaction, which
    /// only contains the newly added data.
    /// For a split event, this corresponds to the individual splits, so the original data does not
    /// get accounted for more than once.
    pub fn delta_chunk(&self) -> &Arc<Chunk> {
        #[expect(clippy::match_same_arms)] // the explicitness is important
        match self.direct_lineage {
            ChunkDirectLineageReport::CompactedFrom(_) => &self.chunk_before_processing,
            ChunkDirectLineageReport::SplitFrom(_, _) => &self.chunk_after_processing,
            _ => &self.chunk_before_processing,
        }
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.chunk_before_processing.is_static()
    }

    /// [`ChunkMeta`] for the `delta_chunk`.
    pub fn chunk_meta(&self) -> ChunkMeta {
        let delta_chunk = self.delta_chunk();
        let entity_path = delta_chunk.entity_path();

        let components: Vec<ChunkComponentMeta> = delta_chunk
            .components()
            .values()
            .map(|column| ChunkComponentMeta {
                descriptor: column.descriptor.clone(),
                inner_arrow_datatype: Some(column.list_array.value_type()),
                has_data: !column.list_array.values().is_empty(),
                is_static_only: delta_chunk.is_static(),
            })
            .collect();

        ChunkMeta {
            entity_path: entity_path.clone(),
            components,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkStoreDiffVirtualAddition {
    /// The [`RrdManifest`] that was passed to [`ChunkStore::insert_rrd_manifest`].
    ///
    /// This is very different from the usual chunk-related events as this is purely virtual.
    /// Still, even though no physical data was ingested yet, it might be important for downstream
    /// consumers to know what kind of virtual data is being referenced from now on.
    /// For example, query caches must know about pending tombstones as soon as they start being referenced.
    ///
    /// If this is set, all fields below are irrelevant (set to their default/empty values).
    pub rrd_manifest: Arc<RrdManifest>,
}

impl ChunkStoreDiffVirtualAddition {
    /// Iterator over [`ChunkMeta`]s in the new rrd manifest.
    ///
    /// In no particular order.
    pub fn chunk_metas(&self) -> impl Iterator<Item = ChunkMeta> {
        re_tracing::profile_function!();

        // Build per-component metadata from the recording's sorbet schema.
        let component_schema_info: ahash::HashMap<
            re_chunk::ComponentIdentifier,
            ChunkComponentMeta,
        > = self
            .rrd_manifest
            .sorbet_schema()
            .fields()
            .iter()
            .filter(|f| {
                re_sorbet::ColumnKind::try_from(f.as_ref()).ok()
                    == Some(re_sorbet::ColumnKind::Component)
            })
            .map(|field| {
                let inner_arrow_datatype = match field.data_type() {
                    arrow::datatypes::DataType::List(inner)
                    | arrow::datatypes::DataType::LargeList(inner) => inner.data_type().clone(),
                    other => other.clone(),
                };

                let descriptor = re_sdk_types::ComponentDescriptor::from((**field).clone());
                (
                    descriptor.component,
                    ChunkComponentMeta {
                        descriptor,
                        inner_arrow_datatype: Some(inner_arrow_datatype),
                        // These fields are filled in later in this function
                        has_data: false,
                        is_static_only: false,
                    },
                )
            })
            .collect();

        /// Helper to track what's know about a component from the manifest's static/temporal maps.
        #[derive(Default)]
        struct VirtualComponentInfo {
            has_temporal: bool,

            has_rows: bool,
        }

        let mut entity_components = ahash::HashMap::<_, nohash_hasher::IntMap<_, _>>::default();

        #[expect(
            clippy::iter_over_hash_type,
            reason = "This collects information into hashmaps"
        )]
        for (entity_path, per_component) in self.rrd_manifest.static_map() {
            let entry = entity_components.entry(entity_path).or_default();
            for &component in per_component.keys() {
                // Static entries always have data (they wouldn't be in the map otherwise).
                entry.insert(
                    component,
                    VirtualComponentInfo {
                        has_temporal: false,
                        has_rows: true,
                    },
                );
            }
        }

        #[expect(
            clippy::iter_over_hash_type,
            reason = "This collects information into hashmaps"
        )]
        for (entity_path, per_timeline) in self.rrd_manifest.temporal_map() {
            let entry = entity_components.entry(entity_path).or_default();
            for per_component in per_timeline.values() {
                for (&component, per_chunk) in per_component {
                    let has_rows = per_chunk.values().any(|e| e.num_rows > 0);

                    let existing = entry.entry(component).or_default();
                    existing.has_temporal = true;
                    existing.has_rows |= has_rows;
                }
            }
        }

        entity_components
            .into_iter()
            .map(move |(entity_path, components)| ChunkMeta {
                entity_path: entity_path.clone(),
                components: components
                    .into_iter()
                    .map(|(component, info)| {
                        let has_data = info.has_rows;
                        let is_static_only = !info.has_temporal;
                        if let Some(meta) = component_schema_info.get(&component) {
                            ChunkComponentMeta {
                                has_data,
                                is_static_only,
                                ..meta.clone()
                            }
                        } else {
                            ChunkComponentMeta {
                                has_data,
                                is_static_only,
                                descriptor: re_sdk_types::ComponentDescriptor::partial(component),
                                inner_arrow_datatype: None,
                            }
                        }
                    })
                    .collect(),
            })
    }
}

/// An atomic deletion event.
///
/// Reminder: ⚠ Do not confuse _a deletion_ and _a clear_ ⚠.
///
/// A deletion is the result of a chunk being completely removed from the store as part of the
/// garbage collection process.
///
/// A clear, on the other hand, is the act of logging an empty [`re_types_core::ComponentBatch`],
/// either directly using the logging APIs, or indirectly through the use of a
/// [`re_types_core::archetypes::Clear`] archetype.
#[derive(Clone)]
pub struct ChunkStoreDiffDeletion {
    /// The chunk that was removed.
    //
    // NOTE: We purposefully use an `Arc` instead of a `ChunkId` here because we want to make sure that all
    // downstream subscribers get a chance to inspect the data in the chunk before it gets permanently
    // deallocated.
    pub chunk: Arc<Chunk>,
}

impl std::fmt::Debug for ChunkStoreDiffDeletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { chunk } = self;
        f.debug_tuple("ChunkStoreDiffDeletion")
            .field(&chunk.id())
            .finish()
    }
}

impl PartialEq for ChunkStoreDiffDeletion {
    fn eq(&self, other: &Self) -> bool {
        let Self { chunk } = self;
        chunk.id() == other.chunk.id()
    }
}

impl ChunkStoreDiffDeletion {
    #[inline]
    pub fn is_static(&self) -> bool {
        self.chunk.is_static()
    }
}

// ---

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use re_chunk::{RowId, TimelineName};
    use re_log_types::example_components::{MyColor, MyIndex, MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline};
    use re_sdk_types::ComponentDescriptor;

    use super::*;
    use crate::{ChunkStore, GarbageCollectionOptions};

    /// A simple store subscriber for test purposes that keeps track of the quantity of data available
    /// in the store at the lowest level of detail.
    ///
    /// The counts represent numbers of rows: e.g. how many unique rows contain this entity path?
    #[derive(Default, Debug, PartialEq, Eq)]
    struct GlobalCounts {
        row_ids: BTreeMap<RowId, i64>,
        timelines: BTreeMap<TimelineName, i64>,
        entity_paths: BTreeMap<EntityPath, i64>,
        component_descrs: BTreeMap<ComponentDescriptor, i64>,
        times: BTreeMap<TimeInt, i64>,
        num_static: i64,
    }

    impl GlobalCounts {
        fn new(
            row_ids: impl IntoIterator<Item = (RowId, i64)>, //
            timelines: impl IntoIterator<Item = (TimelineName, i64)>, //
            entity_paths: impl IntoIterator<Item = (EntityPath, i64)>, //
            component_descrs: impl IntoIterator<Item = (ComponentDescriptor, i64)>, //
            times: impl IntoIterator<Item = (TimeInt, i64)>, //
            num_static: i64,
        ) -> Self {
            Self {
                row_ids: row_ids.into_iter().collect(),
                timelines: timelines.into_iter().collect(),
                entity_paths: entity_paths.into_iter().collect(),
                component_descrs: component_descrs.into_iter().collect(),
                times: times.into_iter().collect(),
                num_static,
            }
        }
    }

    impl GlobalCounts {
        fn on_events(&mut self, events: &[ChunkStoreEvent]) {
            #![expect(clippy::cast_possible_wrap)] // as i64 won't overflow

            for event in events {
                let delta = event.delta();
                let delta_chunk = event.delta_chunk().unwrap();
                let delta_rows = delta * delta_chunk.num_rows() as i64;

                for row_id in delta_chunk.row_ids() {
                    *self.row_ids.entry(row_id).or_default() += delta;
                }
                *self
                    .entity_paths
                    .entry(delta_chunk.entity_path().clone())
                    .or_default() += delta;

                for column in delta_chunk.components().values() {
                    let delta = event.delta() * column.list_array.iter().flatten().count() as i64;
                    *self
                        .component_descrs
                        .entry(column.descriptor.clone())
                        .or_default() += delta;
                }

                if delta_chunk.is_static() {
                    self.num_static += delta_rows;
                } else {
                    for (&timeline, time_column) in delta_chunk.timelines() {
                        *self.timelines.entry(timeline).or_default() += delta_rows;
                        for time in time_column.times() {
                            *self.times.entry(time).or_default() += delta;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn store_events() -> anyhow::Result<()> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            Default::default(),
        );

        let mut view = GlobalCounts::default();

        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_duration("other");
        let timeline_yet_another = Timeline::new_sequence("yet_another");

        let row_id1 = RowId::new();
        let timepoint1 = TimePoint::from_iter([
            (timeline_frame, 42),      //
            (timeline_other, 666),     //
            (timeline_yet_another, 1), //
        ]);
        let entity_path1: EntityPath = "entity_a".into();
        let chunk1 = Chunk::builder(entity_path1.clone())
            .with_component_batch(
                row_id1,
                timepoint1.clone(),
                (MyIndex::partial_descriptor(), &MyIndex::from_iter(0..10)),
            )
            .build()?;

        view.on_events(&store.insert_chunk(&Arc::new(chunk1))?);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                ],
                [
                    (*timeline_frame.name(), 1),
                    (*timeline_other.name(), 1),
                    (*timeline_yet_another.name(), 1),
                ],
                [
                    (entity_path1.clone(), 1), //
                ],
                [
                    (MyIndex::partial_descriptor(), 1), //
                ],
                [
                    (42.try_into().unwrap(), 1), //
                    (666.try_into().unwrap(), 1),
                    (1.try_into().unwrap(), 1),
                ],
                0,
            ),
            view,
        );

        let row_id2 = RowId::new();
        let timepoint2 = TimePoint::from_iter([
            (timeline_frame, 42),      //
            (timeline_yet_another, 1), //
        ]);
        let entity_path2: EntityPath = "entity_b".into();
        let chunk2 = {
            let num_instances = 3;
            let points: Vec<_> = (0..num_instances)
                .map(|i| MyPoint::new(0.0, i as f32))
                .collect();
            let colors = vec![MyColor::from(0xFF0000FF)];
            Chunk::builder(entity_path2.clone())
                .with_component_batches(
                    row_id2,
                    timepoint2.clone(),
                    [
                        (MyPoints::descriptor_points(), &points as _),
                        (MyPoints::descriptor_colors(), &colors as _),
                    ],
                )
                .build()?
        };

        view.on_events(&store.insert_chunk(&Arc::new(chunk2))?);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                    (row_id2, 1),
                ],
                [
                    (*timeline_frame.name(), 2),
                    (*timeline_other.name(), 1),
                    (*timeline_yet_another.name(), 2),
                ],
                [
                    (entity_path1.clone(), 1), //
                    (entity_path2.clone(), 1), //
                ],
                [
                    (MyIndex::partial_descriptor(), 1), // autogenerated, doesn't change
                    (MyPoints::descriptor_points(), 1), //
                    (MyPoints::descriptor_colors(), 1), //
                ],
                [
                    (42.try_into().unwrap(), 2), //
                    (666.try_into().unwrap(), 1),
                    (1.try_into().unwrap(), 2),
                ],
                0,
            ),
            view,
        );

        let row_id3 = RowId::new();
        let timepoint3 = TimePoint::default();
        let chunk3 = {
            let num_instances = 6;
            let colors = vec![MyColor::from(0x00DD00FF); num_instances];
            Chunk::builder(entity_path2.clone())
                .with_component_batches(
                    row_id3,
                    timepoint3.clone(),
                    [
                        (
                            MyIndex::partial_descriptor(),
                            &MyIndex::from_iter(0..num_instances as _) as _,
                        ),
                        (MyPoints::descriptor_colors(), &colors as _),
                    ],
                )
                .build()?
        };

        view.on_events(&store.insert_chunk(&Arc::new(chunk3))?);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 1), //
                    (row_id2, 1),
                    (row_id3, 1),
                ],
                [
                    (*timeline_frame.name(), 2),
                    (*timeline_other.name(), 1),
                    (*timeline_yet_another.name(), 2),
                ],
                [
                    (entity_path1.clone(), 1), //
                    (entity_path2.clone(), 2), //
                ],
                [
                    (MyIndex::partial_descriptor(), 2), //
                    (MyPoints::descriptor_points(), 1), //
                    (MyPoints::descriptor_colors(), 2), //
                ],
                [
                    (42.try_into().unwrap(), 2), //
                    (666.try_into().unwrap(), 1),
                    (1.try_into().unwrap(), 2),
                ],
                1,
            ),
            view,
        );

        let events = store.gc(&GarbageCollectionOptions::gc_everything()).0;
        view.on_events(&events);

        similar_asserts::assert_eq!(
            GlobalCounts::new(
                [
                    (row_id1, 0), //
                    (row_id2, 0),
                    (row_id3, 1), // static -- no gc
                ],
                [
                    (*timeline_frame.name(), 0),
                    (*timeline_other.name(), 0),
                    (*timeline_yet_another.name(), 0),
                ],
                [
                    (entity_path1.clone(), 0), //
                    (entity_path2.clone(), 1), // static -- no gc
                ],
                [
                    (MyIndex::partial_descriptor(), 1), // static -- no gc
                    (MyPoints::descriptor_points(), 0), //
                    (MyPoints::descriptor_colors(), 1), // static -- no gc
                ],
                [
                    (42.try_into().unwrap(), 0), //
                    (666.try_into().unwrap(), 0),
                    (1.try_into().unwrap(), 0),
                ],
                1, // static -- no gc
            ),
            view,
        );

        Ok(())
    }
}
