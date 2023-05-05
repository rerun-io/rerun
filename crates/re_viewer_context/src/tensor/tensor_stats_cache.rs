use re_log_types::{component_types, Tensor};

use super::TensorStats;
use crate::Cache;

#[derive(Default)]
pub struct TensorStatsCache(nohash_hasher::IntMap<component_types::TensorId, TensorStats>);

impl TensorStatsCache {
    pub fn entry(&mut self, tensor: &Tensor) -> &TensorStats {
        self.0
            .entry(tensor.tensor_id)
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
