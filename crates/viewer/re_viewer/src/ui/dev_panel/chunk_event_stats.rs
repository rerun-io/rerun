use std::sync::OnceLock;

use re_chunk_store::{
    ChunkDeletionReason, ChunkStore, ChunkStoreDiff, ChunkStoreEvent, ChunkStoreSubscriberHandle,
    PerStoreChunkSubscriber,
};

/// Per-store statistics derived from chunk store events.
///
/// Registered as a global [`PerStoreChunkSubscriber`].
#[derive(Default, Clone)]
pub struct ChunkEventStats {
    pub num_chunks_gc: u64,
    pub num_chunks_split_cleanup: u64,
    pub num_chunks_compacted: u64,
    pub num_chunks_overwritten: u64,
    pub num_chunks_explicit_drop: u64,
}

impl re_byte_size::MemUsageTreeCapture for ChunkEventStats {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(std::mem::size_of::<Self>() as u64)
    }
}

impl PerStoreChunkSubscriber for ChunkEventStats {
    fn name() -> String {
        "ChunkEventStats".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        for event in events {
            if let ChunkStoreDiff::Deletion(del) = &event.diff {
                match del.reason {
                    ChunkDeletionReason::GarbageCollection => self.num_chunks_gc += 1,
                    ChunkDeletionReason::VirtualToPhysicalReplacement => {
                        // Not interested.
                    }
                    ChunkDeletionReason::DanglingSplitCleanup => {
                        self.num_chunks_split_cleanup += 1;
                    }
                    ChunkDeletionReason::Compaction => self.num_chunks_compacted += 1,
                    ChunkDeletionReason::Overwrite => self.num_chunks_overwritten += 1,
                    ChunkDeletionReason::ExplicitDrop => self.num_chunks_explicit_drop += 1,
                }
            }
        }
    }
}

impl ChunkEventStats {
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Get stats for a specific store.
    pub fn for_store(store_id: &re_log_types::StoreId) -> Self {
        ChunkStore::with_per_store_subscriber_once::<Self, _, _>(
            Self::subscription_handle(),
            store_id,
            |stats| stats.clone(),
        )
        .unwrap_or_default()
    }
}
