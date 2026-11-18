use ahash::{HashMap, HashSet};
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_sdk_types::RowId;
use re_sdk_types::encodings::TensorData;

use crate::{Cache, CacheEntryAccess, TensorStats, VisualizerComponentSource};

/// Caches tensor stats.
#[derive(Default)]
pub struct TensorStatsCache(HashMap<(RowId, VisualizerComponentSource), TensorStats>);

pub struct TensorStatsAccessor<'a> {
    /// NOTE: `TensorData` is never batched (they are mono-components),
    /// so we don't need the instance id here.
    pub row_id: RowId,

    pub source: &'a VisualizerComponentSource,

    /// The tensor data over which we're computing stats. This is needed for the cache miss case.
    pub tensor: &'a TensorData,
}

impl TensorStatsCache {
    pub fn entry(
        &mut self,
        row_id: RowId,
        source: &VisualizerComponentSource,
        tensor: &TensorData,
    ) -> TensorStats {
        // Resolved blueprint values have zeroed row IDs. They cannot identify cached data:
        // changes to an override and distinct overrides would share the same cache key.
        //
        // Default is usually just an empty array, not a problem.
        // SDK-set overrides also use rowId zero and can contain large tensors, their stats are recomputed on every access right now.
        if row_id == RowId::ZERO {
            return TensorStats::from_tensor(tensor);
        }

        *self
            .0
            .entry((row_id, source.clone()))
            .or_insert_with(|| TensorStats::from_tensor(tensor))
    }
}

impl<'a> CacheEntryAccess<TensorStatsAccessor<'a>, TensorStats> for TensorStatsCache {
    fn read(&self, key: &TensorStatsAccessor<'a>) -> Option<TensorStats> {
        self.0.get(&(key.row_id, key.source.clone())).copied()
    }

    fn compute(&mut self, key: &TensorStatsAccessor<'a>) -> TensorStats {
        self.entry(key.row_id, key.source, key.tensor)
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

        let deleted_rows: HashSet<RowId> = events
            .iter()
            .filter_map(|e| e.to_deletion())
            .flat_map(|del| del.chunk.row_ids())
            .collect();

        self.0
            .retain(|(row_id, _source), _stats| !deleted_rows.contains(row_id));
    }
}

impl re_byte_size::MemUsageTreeCapture for TensorStatsCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}
