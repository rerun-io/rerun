use std::sync::Arc;

use egui::ahash::HashMap;
use once_cell::sync::OnceCell;
use re_chunk_store::{
    Chunk, ChunkId, ChunkStore, ChunkStoreEvent, ChunkStoreSubscriber, ChunkStoreSubscriberHandle,
};
use re_log_types::{EntityPath, EntityPathHash, StoreId, Timeline};
use re_viewer_context::external::nohash_hasher::IntMap; // TODO:

// TODO: not done. unclear if I want this.

/// Recursive statistics for a given timeline & entity.
#[derive(Default)]
pub struct EntityTimelineChunks {
    // TODO: this should be a BTreeMap sorted by lowest time on the given timeline for fast range lookups.
    pub recursive_chunks: HashMap<ChunkId, Arc<Chunk>>,
}

// TODO: build up a data-structure that we can retain.
// pub struct ChunkTimelineStats {
//     pub chunk_id: ChunkId,

//     /// How many events there are for the given component for the chunk on the timeline this struct is for.
//     pub num_events_per_component: IntMap<ComponentName, u64>,

//     /// Sum of all items in `Self::num_events_per_component` plus event count of all children
//     pub num_events_recursive: u64,
// }

/// Keeps track of various statistics about chunks that are used to draw the data density graph.
///
/// The store subscription is only used for invalidating existing data rather than building up
/// all statistics continuously.
#[derive(Default)]
pub struct PathRecursiveChunksPerTimeline {
    timeline_chunk_stats: IntMap<Timeline, IntMap<EntityPathHash, EntityTimelineChunks>>,
}

impl PathRecursiveChunksPerTimeline {
    /// Accesses the chunk
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&Self) -> T) -> Option<T> {
        ChunkStore::with_subscriber_once(
            PathRecursiveChunksPerTimelineStoreSubscriber::subscription_handle(),
            move |subscriber: &PathRecursiveChunksPerTimelineStoreSubscriber| {
                subscriber.per_store.get(store_id).map(f)
            },
        )
        .flatten()
    }

    pub fn entity_timeline_stats(
        &self,
        entity_path: &EntityPath,
        timeline: Timeline,
    ) -> Option<&EntityTimelineChunks> {
        self.timeline_chunk_stats
            .get(&timeline)?
            .get(&entity_path.hash())
    }
}

#[derive(Default)]
struct PathRecursiveChunksPerTimelineStoreSubscriber {
    per_store: HashMap<StoreId, PathRecursiveChunksPerTimeline>,
}

impl PathRecursiveChunksPerTimelineStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| ChunkStore::register_subscriber(Box::<Self>::default()))
    }
}

impl ChunkStoreSubscriber for PathRecursiveChunksPerTimelineStoreSubscriber {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscriber.ChunkStatistics".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    #[inline]
    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            let path_recursive_chunks = self.per_store.entry(event.store_id.clone()).or_default();

            if let Some(re_chunk_store::ChunkCompactionReport {
                compacted_chunks,
                new_chunk,
            }) = &event.diff.compacted
            {
                for removed_chunk in compacted_chunks.values() {
                    remove_chunk(path_recursive_chunks, removed_chunk);
                }
                add_chunk(path_recursive_chunks, &new_chunk);
            } else {
                match event.diff.kind {
                    re_chunk_store::ChunkStoreDiffKind::Addition => {
                        add_chunk(path_recursive_chunks, &event.chunk);
                    }
                    re_chunk_store::ChunkStoreDiffKind::Deletion => {
                        remove_chunk(path_recursive_chunks, &event.chunk);
                    }
                }
            }
        }
    }
}

fn add_chunk(path_recursive_chunks: &mut PathRecursiveChunksPerTimeline, chunk: &Arc<Chunk>) {
    re_tracing::profile_function!();

    for timeline in chunk.timelines().keys() {
        let chunks_per_entities = path_recursive_chunks
            .timeline_chunk_stats
            .entry(timeline.clone())
            .or_default();

        // Recursively add chunks.
        let mut next_path = Some(chunk.entity_path().clone());
        while let Some(path) = next_path {
            let chunks_per_entity = chunks_per_entities.entry(path.hash()).or_default();
            chunks_per_entity
                .recursive_chunks
                .insert(chunk.id(), chunk.clone());
            next_path = path.parent();
        }
    }
}

fn remove_chunk(path_recursive_chunks: &mut PathRecursiveChunksPerTimeline, chunk: &Chunk) {
    re_tracing::profile_function!();

    for timeline in chunk.timelines().keys() {
        let Some(chunks_per_entities) =
            path_recursive_chunks.timeline_chunk_stats.get_mut(timeline)
        else {
            continue;
        };

        // Recursively remove chunks.
        let mut next_path = Some(chunk.entity_path().clone());
        while let Some(path) = next_path {
            if let Some(chunks_per_entity) = chunks_per_entities.get_mut(&path.hash()) {
                chunks_per_entity.recursive_chunks.remove(&chunk.id());
            }
            next_path = path.parent();
        }
    }
}

// TODO: add unit tests
