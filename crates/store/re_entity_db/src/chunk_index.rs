use std::collections::hash_map::Entry;

use ahash::{HashMap, HashSet};
use itertools::Itertools as _;

use re_chunk::ChunkId;
use re_chunk_store::ChunkStoreEvent;
use re_types_core::ChunkIndexMessage;

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct ChunkInfo {
    /// Do we have the whole chunk in memory?
    pub fully_loaded: bool,
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is currently used to show a progress bar.
///
/// This is constructed from one ore more [`ChunkIndexMessage`], which is what
/// the server sends to the client/viewer.
/// TODO(RR-2999): use this for larger-than-RAM.
#[derive(Default, Debug, Clone)]
pub struct ChunkIndex {
    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantally.
    remote_chunks: HashMap<ChunkId, ChunkInfo>,

    /// The chunk store may split large chunks and merge (compact) small ones.
    /// When we later drop a chunk, we need to know which other chunks to invalidate.
    parents: HashMap<ChunkId, HashSet<ChunkId>>,

    /// Have we ever deleted a chunk?
    ///
    /// If so, we have run some GC and should not show progress bar.
    has_deleted: bool,
}

impl ChunkIndex {
    #[expect(clippy::needless_pass_by_value)] // In the future we may want to store them as record batches
    pub fn append(&mut self, msg: ChunkIndexMessage) {
        re_tracing::profile_function!();
        for chunk_id in msg.chunk_ids() {
            match self.remote_chunks.entry(*chunk_id) {
                Entry::Occupied(_occupied_entry) => {
                    // TODO(RR-2999): update time range index for the chunk
                }
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(ChunkInfo {
                        fully_loaded: false,
                    });
                }
            }
        }
    }

    /// How many chunks are in the index?
    ///
    /// Not all of them are necessarily loaded.
    pub fn num_chunks(&self) -> usize {
        self.remote_chunks.len()
    }

    /// [0, 1], how many chunks have been loaded?
    ///
    /// Returns `None` if we have already started garbage-collecting some chunks.
    pub fn progress(&self) -> Option<f32> {
        if self.has_deleted {
            None
        } else if self.num_chunks() == 0 {
            Some(1.0)
        } else {
            let num_loaded = self
                .remote_chunks
                .values()
                .filter(|c| c.fully_loaded)
                .count();
            Some(num_loaded as f32 / self.num_chunks() as f32)
        }
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
        chunk_info.fully_loaded = true;
    }

    pub fn on_events(&mut self, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in store_events {
            let chunk_id = event.chunk.id();
            match event.kind {
                re_chunk_store::ChunkStoreDiffKind::Addition => {
                    if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                        chunk_info.fully_loaded = true;
                    } else if let Some(source) = event.split_source {
                        // The added chunk was the result of splitting another chunk:
                        self.parents.entry(chunk_id).or_default().insert(source);
                    } else {
                        re_log::warn!("Added chunk that was not part of the index");
                    }
                }
                re_chunk_store::ChunkStoreDiffKind::Deletion => {
                    self.mark_deleted(&chunk_id);
                }
            }
        }
    }

    fn mark_deleted(&mut self, chunk_id: &ChunkId) {
        self.has_deleted = true;

        if let Some(chunk_info) = self.remote_chunks.get_mut(chunk_id) {
            chunk_info.fully_loaded = false;
        } else if let Some(parents) = self.parents.remove(chunk_id) {
            // Mark all ancestors as not being fully loaded:

            let mut ancestors = parents.into_iter().collect_vec();
            while let Some(chunk_id) = ancestors.pop() {
                if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                    chunk_info.fully_loaded = false;
                } else if let Some(grandparents) = self.parents.get(&chunk_id) {
                    ancestors.extend(grandparents);
                } else {
                    re_log::warn!("Removed chunk that was not part of the index");
                }
            }
        } else {
            re_log::warn!("Removed chunk that was not part of the index");
        }
    }
}
