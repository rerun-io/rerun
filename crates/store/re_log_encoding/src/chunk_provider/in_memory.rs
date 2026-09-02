use std::sync::Arc;

use ahash::HashMap;

use re_chunk::{Chunk, ChunkId};
use re_log_types::StoreId;

use crate::{ChunkProvider, ChunkProviderError, CodecResult, RawRrdManifest, RrdManifest};

/// [`ChunkProvider`] over a set of already-materialized chunks.
pub struct InMemoryChunkProvider {
    chunks: HashMap<ChunkId, Arc<Chunk>>,
    manifest: Arc<RrdManifest>,
    raw_manifest: Arc<RawRrdManifest>,

    /// Human-readable source identifier for diagnostics.
    source: String,
}

impl InMemoryChunkProvider {
    pub fn new(
        store_id: &StoreId,
        chunks: impl IntoIterator<Item = Arc<Chunk>>,
    ) -> CodecResult<Self> {
        // Build the manifest from the caller's insertion order: the synthesized byte offsets and
        // the file-order sweep of downstream consumers must be deterministic, which a hash-map
        // iteration is not.
        let chunks: Vec<Arc<Chunk>> = chunks.into_iter().collect();
        let raw_manifest = Arc::new(RawRrdManifest::build_in_memory_from_chunks(
            store_id.clone(),
            chunks.iter().map(AsRef::as_ref),
        )?);
        let manifest = Arc::new(RrdManifest::try_new(&raw_manifest)?);
        let chunks: HashMap<_, _> = chunks
            .into_iter()
            .map(|chunk| (chunk.id(), chunk))
            .collect();

        Ok(Self {
            chunks,
            manifest,
            raw_manifest,
            source: format!("in-memory store {store_id}"),
        })
    }
}

#[derive(thiserror::Error, Debug)]
#[error("unknown chunk id: {0}")]
struct UnknownChunkIdError(ChunkId);

#[async_trait::async_trait]
impl ChunkProvider for InMemoryChunkProvider {
    fn manifest(&self) -> &Arc<RrdManifest> {
        &self.manifest
    }

    fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        &self.raw_manifest
    }

    fn source(&self) -> String {
        self.source.clone()
    }

    async fn load_chunks(&self, ids: &[ChunkId]) -> Result<Vec<Arc<Chunk>>, ChunkProviderError> {
        ids.iter()
            .map(|id| {
                self.chunks
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ChunkProviderError(Box::new(UnknownChunkIdError(*id))))
            })
            .collect()
    }
}
