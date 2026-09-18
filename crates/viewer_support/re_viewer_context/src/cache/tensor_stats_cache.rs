use ahash::{HashMap, HashSet};
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_sdk_types::encodings::TensorData;
use re_sdk_types::{ComponentIdentifier, RowId};

use crate::{Cache, CacheEntryAccess, TensorStats};

/// Caches tensor stats.
#[derive(Default)]
pub struct TensorStatsCache(HashMap<(RowId, ComponentIdentifier), TensorStats>);

pub struct TensorStatsAccessor<'a> {
    /// NOTE: `TensorData` is never batched (they are mono-components),
    /// so we don't need the instance id here.
    pub row_id: RowId,

    pub component: ComponentIdentifier,

    /// The tensor data over which we're computing stats. This is needed for the cache miss case.
    pub tensor: &'a TensorData,
}

impl TensorStatsCache {
    pub fn entry(
        &mut self,
        row_id: RowId,
        component: ComponentIdentifier,
        tensor: &TensorData,
    ) -> TensorStats {
        *self
            .0
            .entry((row_id, component))
            .or_insert_with(|| TensorStats::from_tensor(tensor))
    }
}

impl<'a> CacheEntryAccess<TensorStatsAccessor<'a>, TensorStats> for TensorStatsCache {
    fn read(&self, key: &TensorStatsAccessor<'a>) -> Option<TensorStats> {
        self.0.get(&(key.row_id, key.component)).copied()
    }

    fn compute(&mut self, key: &TensorStatsAccessor<'a>) -> TensorStats {
        self.entry(key.row_id, key.component, key.tensor)
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
            .retain(|(row_id, _component), _stats| !deleted_rows.contains(row_id));
    }
}

impl re_byte_size::MemUsageTreeCapture for TensorStatsCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}
