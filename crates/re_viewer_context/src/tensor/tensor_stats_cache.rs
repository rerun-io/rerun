use re_log_types::RowId;
use re_types::datatypes::TensorData;

use super::TensorStats;
use crate::Cache;

/// Caches tensor stats using a [`RowId`], i.e. a specific instance of
/// a `TensorData` component
#[derive(Default)]
pub struct TensorStatsCache(ahash::HashMap<RowId, TensorStats>);

impl TensorStatsCache {
    /// The key should be the `RowId` of the `TensorData`.
    /// NOTE: `TensorData` is never batched (they are mono-components),
    /// so we don't need the instance id here.
    pub fn entry(&mut self, key: RowId, tensor: &TensorData) -> TensorStats {
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
