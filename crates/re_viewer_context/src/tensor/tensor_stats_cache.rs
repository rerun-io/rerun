use re_data_store::VersionedInstancePathHash;
use re_log_types::RowId;
use re_types::datatypes::TensorData;

use super::TensorStats;
use crate::Cache;

/// Caches tensor stats using a [`RowId`], i.e. a specific instance of
/// a `TensorData` component
#[derive(Default)]
pub struct TensorStatsCache(ahash::HashMap<RowId, TensorStats>);

impl TensorStatsCache {
    pub fn entry(&mut self, key: VersionedInstancePathHash, tensor: &TensorData) -> TensorStats {
        re_tracing::profile_function!();
        let key = key.row_id;

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
