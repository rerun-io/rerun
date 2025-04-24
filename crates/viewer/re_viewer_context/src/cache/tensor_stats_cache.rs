use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_types::{datatypes::TensorData, Component as _};

use crate::{Cache, TensorStats};

/// Caches tensor stats.
///
/// Use [`re_types_core::RowId`] as cache key when available.
#[derive(Default)]
pub struct TensorStatsCache(HashMap<Hash64, TensorStats>);

impl TensorStatsCache {
    /// The `RowId` of the `TensorData` may be used as a cache key.
    /// NOTE: `TensorData` is never batched (they are mono-components),
    /// so we don't need the instance id here.
    pub fn entry(&mut self, tensor_cache_key: Hash64, tensor: &TensorData) -> TensorStats {
        *self
            .0
            .entry(tensor_cache_key)
            .or_insert_with(|| TensorStats::from_tensor(tensor))
    }
}

impl Cache for TensorStatsCache {
    fn purge_memory(&mut self) {
        // Purging the tensor stats is not worth it - these are very small objects!
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let cache_keys: HashSet<Hash64> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_tensor_data = || {
                    event
                        .chunk
                        .components()
                        .contains_key(&re_types::components::TensorData::name())
                };

                if is_deletion() && contains_tensor_data() {
                    Either::Left(event.chunk.row_ids().map(Hash64::hash))
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|cache_key, _per_key| !cache_keys.contains(cache_key));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
