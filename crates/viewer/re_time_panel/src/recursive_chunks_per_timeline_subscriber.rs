use std::sync::{Arc, OnceLock};

use egui::ahash::HashMap;
use nohash_hasher::IntMap;
use re_chunk_store::{
    Chunk, ChunkDirectLineageReport, ChunkId, ChunkStore, ChunkStoreEvent,
    ChunkStoreSubscriberHandle, PerStoreChunkSubscriber,
};
use re_log_types::{AbsoluteTimeRange, EntityPath, EntityPathHash, StoreId, TimelineName};

/// Cached information about a chunk in the context of a given timeline.
#[derive(Debug, Clone)]
pub struct ChunkTimelineInfo {
    pub chunk: Arc<Chunk>,
    pub num_events: u64,
    pub resolved_time_range: AbsoluteTimeRange,
}

#[cfg(test)]
impl PartialEq for ChunkTimelineInfo {
    fn eq(&self, other: &Self) -> bool {
        self.chunk.id() == other.chunk.id()
            && self.num_events == other.num_events
            && self.resolved_time_range == other.resolved_time_range
    }
}

/// Recursive chunk timeline infos for a given timeline & entity.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EntityTimelineChunks {
    /// All chunks used by the entity & timeline, recursive for all children of the entity.
    // TODO(andreas): Sorting this by time range would be great as it would allow us to slice ranges.
    pub recursive_chunks_info: HashMap<ChunkId, ChunkTimelineInfo>,

    /// Total number of events in all [`Self::recursive_chunks_info`] chunks.
    pub total_num_events: u64,
}

/// For each entity & timeline, keeps track of all its chunks and chunks of its children.
#[derive(Default)]
pub struct PathRecursiveChunksPerTimelineStoreSubscriber {
    chunks_per_timeline_per_entity:
        IntMap<TimelineName, IntMap<EntityPathHash, EntityTimelineChunks>>,
}

impl PathRecursiveChunksPerTimelineStoreSubscriber {
    pub fn ensure_registered() {
        Self::subscription_handle();
    }

    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Accesses the chunk
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&Self) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber_once(Self::subscription_handle(), store_id, f)
    }

    pub fn path_recursive_chunks_for_entity_and_timeline(
        &self,
        entity_path: &EntityPath,
        timeline: &TimelineName,
    ) -> Option<&EntityTimelineChunks> {
        self.chunks_per_timeline_per_entity
            .get(timeline)?
            .get(&entity_path.hash())
    }

    fn add_chunk(&mut self, chunk: &Arc<Chunk>) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for (timeline, time_column) in chunk.timelines() {
            let chunks_per_entities = self
                .chunks_per_timeline_per_entity
                .entry(*timeline)
                .or_default();

            let chunk_info = ChunkTimelineInfo {
                chunk: chunk.clone(),
                num_events: chunk.num_events_cumulative(), // TODO(andreas): Would `num_events_cumulative_per_unique_time` be more appropriate?
                resolved_time_range: time_column.time_range(),
            };

            // Recursively add chunks.
            let mut next_path = Some(chunk.entity_path().clone());
            while let Some(path) = next_path {
                let chunks_per_entity = chunks_per_entities.entry(path.hash()).or_default();

                chunks_per_entity
                    .recursive_chunks_info
                    .insert(chunk.id(), chunk_info.clone());
                chunks_per_entity.total_num_events += chunk_info.num_events;
                next_path = path.parent();
            }
        }
    }

    fn remove_chunk(&mut self, chunk: &Chunk) {
        #[expect(clippy::iter_over_hash_type)]
        for timeline in chunk.timelines().keys() {
            let Some(chunks_per_entities) = self.chunks_per_timeline_per_entity.get_mut(timeline)
            else {
                continue;
            };

            // Recursively remove chunks.
            let mut next_path = Some(chunk.entity_path().clone());
            while let Some(path) = next_path {
                if let Some(chunks_per_entity) = chunks_per_entities.get_mut(&path.hash())
                    && chunks_per_entity
                        .recursive_chunks_info
                        .remove(&chunk.id())
                        .is_some()
                {
                    if let Some(new_total_num_events) = chunks_per_entity
                        .total_num_events
                        .checked_sub(chunk.num_events_cumulative())
                    {
                        chunks_per_entity.total_num_events = new_total_num_events;
                    } else {
                        re_log::error_once!(
                            "Total number of recursive events for {:?} for went negative",
                            path
                        );
                    }
                }
                next_path = path.parent();
            }
        }
    }
}

impl PerStoreChunkSubscriber for PathRecursiveChunksPerTimelineStoreSubscriber {
    #[inline]
    fn name() -> String {
        "rerun.store_subscriber.PathRecursiveChunksPerTimeline".into()
    }

    #[inline]
    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            if let Some(ChunkDirectLineageReport::CompactedFrom(chunks)) =
                &event.diff.direct_lineage
            {
                for removed_chunk in chunks.values() {
                    self.remove_chunk(removed_chunk);
                }
                self.add_chunk(&event.diff.chunk_after_processing);
            } else {
                match event.diff.kind {
                    re_chunk_store::ChunkStoreDiffKind::Addition => {
                        self.add_chunk(&event.chunk_after_processing);
                    }
                    re_chunk_store::ChunkStoreDiffKind::Deletion => {
                        self.remove_chunk(&event.chunk_after_processing);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::{Chunk, ChunkStore, ChunkStoreConfig, GarbageCollectionOptions, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{AbsoluteTimeRange, StoreId, TimeInt, Timeline, TimelineName};

    use super::{EntityTimelineChunks, PathRecursiveChunksPerTimelineStoreSubscriber};

    #[test]
    fn path_recursive_chunks_per_timeline() -> anyhow::Result<()> {
        let mut store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED, // Makes it hard to predict chunks otherwise.
        );
        // Initialize the store subscriber. Need to do this ahead of time, otherwise it will miss on events.
        let _subscriber = PathRecursiveChunksPerTimelineStoreSubscriber::subscription_handle();

        // We use two timelines for which we log events on two entities, at the root and at a grandchild.
        let t0 = Timeline::new_sequence("time0");
        let t1 = Timeline::new_sequence("time1");
        let component_batch = (
            MyPoints::descriptor_points(),
            &[MyPoint::new(3.0, 3.0)] as _, // Generic component batch, don't care about the contents.
        );

        // Events at the root path.
        // 2x: single chunk with two events for both t0 and t1.
        for i in 1..=2 {
            store.insert_chunk(&Arc::new(
                Chunk::builder("/")
                    .with_component_batches(
                        RowId::new(),
                        [(t0, i), (t1, i)],
                        [component_batch.clone()],
                    )
                    .with_component_batches(
                        RowId::new(),
                        [(t0, i + 2), (t1, i + 2)],
                        [component_batch.clone()],
                    )
                    .build()?,
            ))?;
        }

        // Events at a child path.
        // One chunk with one event at t0, one chunk with two events at t1
        store.insert_chunk(&Arc::new(
            Chunk::builder("/parent/child")
                .with_component_batches(RowId::new(), [(t0, 0)], [component_batch.clone()])
                .build()?,
        ))?;
        store.insert_chunk(&Arc::new(
            Chunk::builder("/parent/child")
                .with_component_batches(RowId::new(), [(t1, 1)], [component_batch.clone()])
                .with_component_batches(RowId::new(), [(t1, 3)], [component_batch])
                .build()?,
        ))?;

        assert_eq!(
            PathRecursiveChunksPerTimelineStoreSubscriber::access(&store.id(), |subs| {
                test_subscriber_status_before_removal(subs, *t0.name(), *t1.name())
            }),
            Some(Some(()))
        );

        // Remove only the t0 chunk on "parent/child"
        store.gc(&GarbageCollectionOptions {
            protected_time_ranges: [
                (*t0.name(), AbsoluteTimeRange::new(1, TimeInt::MAX)),
                (*t1.name(), AbsoluteTimeRange::EVERYTHING),
            ]
            .into_iter()
            .collect(),
            ..GarbageCollectionOptions::gc_everything()
        });

        assert_eq!(
            PathRecursiveChunksPerTimelineStoreSubscriber::access(&store.id(), |subs| {
                test_subscriber_status_after_t0_child_chunk_removal(subs, *t0.name(), *t1.name())
            }),
            Some(Some(()))
        );

        Ok(())
    }

    fn test_subscriber_status_before_removal(
        subs: &PathRecursiveChunksPerTimelineStoreSubscriber,
        t0: TimelineName,
        t1: TimelineName,
    ) -> Option<()> {
        // The root accumulates all chunks & events for each timeline.
        let root_t0 = subs.path_recursive_chunks_for_entity_and_timeline(&"/".into(), &t0)?;
        assert_eq!(root_t0.recursive_chunks_info.len(), 2 + 1);
        assert_eq!(root_t0.total_num_events, 2 * 2 + 1);
        let root_t1 = subs.path_recursive_chunks_for_entity_and_timeline(&"/".into(), &t1)?;
        assert_eq!(root_t1.recursive_chunks_info.len(), 2 + 1);
        assert_eq!(root_t1.total_num_events, 2 * 2 + 2);

        let child_t0 =
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent/child".into(), &t0)?;
        assert_eq!(child_t0.recursive_chunks_info.len(), 1);
        assert_eq!(child_t0.total_num_events, 1);
        let child_t1 =
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent/child".into(), &t1)?;
        assert_eq!(child_t1.recursive_chunks_info.len(), 1);
        assert_eq!(child_t1.total_num_events, 2);

        test_paths_without_chunks(subs, child_t0, child_t1, t0, t1)?;

        Some(())
    }

    fn test_subscriber_status_after_t0_child_chunk_removal(
        subs: &PathRecursiveChunksPerTimelineStoreSubscriber,
        t0: TimelineName,
        t1: TimelineName,
    ) -> Option<()> {
        // The root accumulates all chunks & events for each timeline.
        let root_t0 = subs.path_recursive_chunks_for_entity_and_timeline(&"/".into(), &t0)?;
        assert_eq!(root_t0.recursive_chunks_info.len(), 2);
        assert_eq!(root_t0.total_num_events, 2 * 2);
        let root_t1 = subs.path_recursive_chunks_for_entity_and_timeline(&"/".into(), &t1)?;
        assert_eq!(root_t1.recursive_chunks_info.len(), 2 + 1);
        assert_eq!(root_t1.total_num_events, 2 * 2 + 2);

        let child_t0 =
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent/child".into(), &t0)?;
        assert_eq!(child_t0.recursive_chunks_info.len(), 0);
        assert_eq!(child_t0.total_num_events, 0);
        let child_t1 =
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent/child".into(), &t1)?;
        assert_eq!(child_t1.recursive_chunks_info.len(), 1);
        assert_eq!(child_t1.total_num_events, 2);

        test_paths_without_chunks(subs, child_t0, child_t1, t0, t1)?;

        Some(())
    }

    fn test_paths_without_chunks(
        subs: &PathRecursiveChunksPerTimelineStoreSubscriber,
        child_t0: &EntityTimelineChunks,
        child_t1: &EntityTimelineChunks,
        t0: TimelineName,
        t1: TimelineName,
    ) -> Option<()> {
        // We only logged at `parent/child`, so we expect all events to `parent` copies over everything `parent/child` has.
        assert_eq!(
            child_t0,
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent".into(), &t0)?
        );
        assert_eq!(
            child_t1,
            subs.path_recursive_chunks_for_entity_and_timeline(&"/parent".into(), &t1)?
        );

        // No information arbitrary down the tree.
        assert!(
            subs.path_recursive_chunks_for_entity_and_timeline(
                &"/parent/child/grandchild".into(),
                &t1
            )
            .is_none()
        );

        Some(())
    }
}
