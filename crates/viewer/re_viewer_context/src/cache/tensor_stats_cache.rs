use ahash::{HashMap, HashSet};
use itertools::Either;
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_sdk_types::archetypes::Tensor;
use re_sdk_types::datatypes::TensorData;

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
    fn name(&self) -> &'static str {
        "TensorStatsCache"
    }

    fn purge_memory(&mut self) {
        // Purging the tensor stats is not worth it - these are very small objects!
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_keys: HashSet<Hash64> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_tensor_data = || {
                    event
                        .chunk_before_processing
                        .components()
                        .contains_component(Tensor::descriptor_data().component)
                };

                if is_deletion() && contains_tensor_data() {
                    Either::Left(event.chunk_before_processing.row_ids().map(Hash64::hash))
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|cache_key, _per_key| !cache_keys.contains(cache_key));
    }
}

impl re_byte_size::MemUsageTreeCapture for TensorStatsCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}
