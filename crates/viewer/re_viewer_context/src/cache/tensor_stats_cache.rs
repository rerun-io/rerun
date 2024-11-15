use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_types::{datatypes::TensorData, Component as _};

use crate::{Cache, TensorStats};

/// Caches tensor stats using a [`RowId`], i.e. a specific instance of
/// a `TensorData` component
#[derive(Default)]
pub struct TensorStatsCache(HashMap<RowId, TensorStats>);

impl TensorStatsCache {
    /// The key should be the `RowId` of the `TensorData`.
    /// NOTE: `TensorData` is never batched (they are mono-components),
    /// so we don't need the instance id here.
    pub fn entry(&mut self, tensor_data_row_id: RowId, tensor: &TensorData) -> TensorStats {
        *self
            .0
            .entry(tensor_data_row_id)
            .or_insert_with(|| TensorStats::from_tensor(tensor))
    }
}

impl Cache for TensorStatsCache {
    fn purge_memory(&mut self) {
        // Purging the tensor stats is not worth it - these are very small objects!
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let row_ids_removed: HashSet<RowId> = events
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
                    Either::Left(event.chunk.row_ids())
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|row_id, _per_key| !row_ids_removed.contains(row_id));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
