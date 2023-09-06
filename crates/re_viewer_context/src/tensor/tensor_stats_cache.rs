use re_components::Tensor;
use re_data_store::VersionedInstancePathHash;

use super::TensorStats;
use crate::Cache;

/// Caches tensor stats using a [`VersionedInstancePathHash`], i.e. a specific instance of
/// a specific entity path for a specific row in the store.
#[derive(Default)]
pub struct TensorStatsCache(ahash::HashMap<VersionedInstancePathHash, TensorStats>);

impl TensorStatsCache {
    pub fn entry(&mut self, key: VersionedInstancePathHash, tensor: &Tensor) -> TensorStats {
        *self
            .0
            .entry(key)
            .or_insert_with(|| TensorStats::new(tensor))
    }
}

impl Cache for TensorStatsCache {
    fn begin_frame(&mut self) {}

    fn purge_memory(&mut self) {
        // Purging the tensor stats is not worth it - these are very small objects!
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
