use re_log_types::StoreId;

/// Encapsulates transform knowledge about a [`re_chunk_store::ChunkStore`].
pub struct TransformDb {
    /// The [`re_chunk_store::ChunkStore`] that this [`TransformDb`] is associated with.
    store_id: StoreId,
}

impl TransformDb {
    pub fn new(store_id: StoreId) -> Self {
        Self { store_id }
    }
}
