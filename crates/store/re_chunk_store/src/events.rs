use std::sync::Arc;

use re_chunk::Chunk;
use re_log_encoding::RrdManifest;
use re_log_types::StoreId;

use crate::{ChunkDirectLineageReport, ChunkStoreGeneration};

#[expect(unused_imports, clippy::unused_trait_names)] // used in docstrings
use crate::{ChunkId, ChunkStore, ChunkStoreSubscriber, RowId};

// ---

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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Is it an addition or a deletion?
///
/// Reminder: ⚠ Do not confuse _a deletion_ and _a clear_ ⚠.
///
/// A deletion is the result of a chunk being completely removed from the store as part of the
/// garbage collection process.
///
/// A clear, on the other hand, is the act of logging an empty [`re_types_core::ComponentBatch`],
/// either directly using the logging APIs, or indirectly through the use of a
/// [`re_types_core::archetypes::Clear`] archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStoreDiffKind {
    Addition,
    Deletion,
}

impl ChunkStoreDiffKind {
    #[inline]
    pub fn delta(&self) -> i64 {
        match self {
            Self::Addition => 1,
            Self::Deletion => -1,
        }
    }
}

/// Describes an atomic change in the Rerun [`ChunkStore`]: a chunk has been added or deleted.
///
/// From a query model standpoint, the [`ChunkStore`] _always_ operates one chunk at a time:
/// - The contents of a chunk (i.e. its columns) are immutable past insertion, by virtue of
///   [`ChunkId`]s being unique and non-reusable.
/// - Similarly, garbage collection always removes _all the data_ associated with a chunk in one go:
///   there cannot be orphaned columns. When a chunk is gone, all data associated with it is gone too.
///
/// Refer to field-level documentation for more information.
#[derive(Debug, Clone)]
pub struct ChunkStoreDiff {
    /// Addition or deletion?
    ///
    /// The store's internals are opaque and don't necessarily reflect the query model (e.g. there
    /// might be data in the store that cannot by reached by any query).
    ///
    /// A [`ChunkStoreDiff`] answers a logical question: "does there exist a query path which can return
    /// data from that chunk?".
    ///
    /// An event of kind deletion only tells you that, from this point on, no query can return data from that chunk.
    /// That doesn't necessarily mean that the data is actually gone, i.e. don't make assumptions of e.g. the size
    /// in bytes of the store based on these events.
    /// They are in "query-model space" and are not an accurate representation of what happens in storage space.
    pub kind: ChunkStoreDiffKind,

    /// The [`RrdManifest`] that was passed to [`ChunkStore::insert_rrd_manifest`].
    ///
    /// This is very different from the usual chunk-related events as this is purely virtual.
    /// Still, even though no physical data was ingested yet, it might be important for downstream
    /// consumers to know what kind of virtual data is being referenced from now on.
    /// For example, query caches must know about pending tombstones as soon as they start being referenced.
    ///
    /// If this is set, all fields below are irrelevant (set to their default/empty values).
    pub rrd_manifest: Option<Arc<RrdManifest>>,

    /// The chunk that was added or removed, *unaltered*.
    ///
    /// This is the chunk exactly as it was passed to the insertion/removal method, before any kind
    /// of processing happened to it (compaction, splitting, etc).
    /// To access the compacted/split data, refer to [`ChunkStoreDiff::chunk_after_processing`] instead.
    ///
    /// ## Relationship to [`ChunkStoreDiff::direct_lineage`]
    ///
    /// If the lineage is…:
    /// * `SplitFrom`: then this is the original chunk, before splitting.
    /// * `CompactedFrom`: then this is the original chunk, before compaction.
    /// * anything else: then is the original chunk.
    ///
    /// In case of deletions, then this is the chunk being deleted, as-is, regardless of its lineage.
    //
    // NOTE: We purposefully use an `Arc` instead of a `ChunkId` here because we want to make sure that all
    // downstream subscribers get a chance to inspect the data in the chunk before it gets permanently
    // deallocated.
    pub chunk_before_processing: Arc<Chunk>,

    /// The chunk that was added or removed, including post-processing (splitting, compaction, etc).
    ///
    /// This is the chunk exactly as it was when it was finally indexed by the store, after all kinds
    /// of processing happened to it (compaction, splitting, etc).
    /// To access the unprocessed data, refer to [`ChunkStoreDiff::chunk_before_processing`] instead.
    ///
    /// ## Relationship to [`ChunkStoreDiff::direct_lineage`]
    ///
    /// If the lineage is…:
    /// * `SplitFrom`: then this is one of split siblings.
    /// * `CompactedFrom`: then this is the compacted chunk.
    /// * anything else: then this is the original chunk, same as [`ChunkStoreDiff::chunk_before_processing`].
    ///
    /// In case of deletions, then this is the chunk being deleted, as-is, regardless of its lineage.
    /// I.e. it's the exact same as [`ChunkStoreDiff::chunk_before_processing`] in that case.
    //
    // NOTE: We purposefully use an `Arc` instead of a `ChunkId` here because we want to make sure that all
    // downstream subscribers get a chance to inspect the data in the chunk before it gets permanently
    // deallocated.
    pub chunk_after_processing: Arc<Chunk>,

    /// The direct lineage of [`ChunkStoreDiff::chunk_after_processing`].
    ///
    /// This can be used to keep track of compactions and split-offs.
    pub direct_lineage: Option<ChunkDirectLineageReport>,
}

impl PartialEq for ChunkStoreDiff {
    #[inline]
    fn eq(&self, rhs: &Self) -> bool {
        let Self {
            kind,
            chunk_before_processing,
            chunk_after_processing,
            rrd_manifest,
            direct_lineage,
        } = self;
        *kind == rhs.kind
            && chunk_before_processing.id() == rhs.chunk_before_processing.id()
            && chunk_after_processing.id() == rhs.chunk_after_processing.id()
            && rrd_manifest == &rhs.rrd_manifest
            && direct_lineage == &rhs.direct_lineage
    }
}

impl Eq for ChunkStoreDiff {}

impl ChunkStoreDiff {
    #[inline]
    pub fn addition(
        chunk_before_processing: Arc<Chunk>,
        chunk_after_processing: Arc<Chunk>,
        direct_lineage: ChunkDirectLineageReport,
    ) -> Self {
        Self {
            kind: ChunkStoreDiffKind::Addition,
            rrd_manifest: None,
            chunk_before_processing,
            chunk_after_processing,
            direct_lineage: Some(direct_lineage),
        }
    }

    pub fn rrd_manifest(rrd_manifest: Arc<RrdManifest>) -> Self {
        Self {
            kind: ChunkStoreDiffKind::Addition,
            rrd_manifest: Some(rrd_manifest),
            chunk_before_processing: Arc::new(Chunk::empty(ChunkId::ZERO, "<empty>".into())),
            chunk_after_processing: Arc::new(Chunk::empty(ChunkId::ZERO, "<empty>".into())),
            direct_lineage: None,
        }
    }

    #[inline]
    pub fn deletion(chunk: Arc<Chunk>) -> Self {
        Self {
            kind: ChunkStoreDiffKind::Deletion,
            rrd_manifest: None,
            chunk_before_processing: chunk.clone(),
            chunk_after_processing: chunk,
            direct_lineage: None,
        }
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.chunk_before_processing.is_static()
    }

    /// `-1` for deletions, `+1` for additions.
    #[inline]
    pub fn delta(&self) -> i64 {
        self.kind.delta()
    }

    #[inline]
    pub fn num_components(&self) -> usize {
        self.chunk_before_processing.num_components()
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
                let delta_chunks = event.delta();
                let delta_rows = delta_chunks * event.chunk_before_processing.num_rows() as i64;

                for row_id in event.chunk_before_processing.row_ids() {
                    *self.row_ids.entry(row_id).or_default() += delta_chunks;
                }
                *self
                    .entity_paths
                    .entry(event.chunk_before_processing.entity_path().clone())
                    .or_default() += delta_chunks;

                for column in event.chunk_before_processing.components().values() {
                    let delta = event.delta() * column.list_array.iter().flatten().count() as i64;
                    *self
                        .component_descrs
                        .entry(column.descriptor.clone())
                        .or_default() += delta;
                }

                if event.is_static() {
                    self.num_static += delta_rows;
                } else {
                    for (&timeline, time_column) in event.chunk_before_processing.timelines() {
                        *self.timelines.entry(timeline).or_default() += delta_rows;
                        for time in time_column.times() {
                            *self.times.entry(time).or_default() += delta_chunks;
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
