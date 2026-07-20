use std::sync::Arc;

use ahash::HashMap;
use tokio_stream::StreamExt as _;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::{ChunkProvider, ChunkProviderError, RawRrdManifest, RrdManifest};
use re_log_types::EntryId;
use re_types_core::SegmentId;
use re_uri::Origin;

use crate::{ApiError, ConnectionRegistryHandle, fetch_chunks_response_to_chunk_and_segment_id};

/// gRPC-backed [`ChunkProvider`]: serves the manifest of a single dataset
/// segment and fetches its chunks on demand via `FetchChunks`.
//TODO(RR-4546): this needs to be on par with the table provider stuff in terms of chunk downloading
// (signed url, batching, etc.). The current streaming strategy is really poor, and only works
// because of a workaround we have to mitigate RR-4545
pub struct SegmentChunkProvider {
    connection_registry: ConnectionRegistryHandle,
    origin: Origin,
    dataset_id: EntryId,
    segment_id: SegmentId,

    raw_manifest: Arc<RawRrdManifest>,
    manifest: Arc<RrdManifest>,

    /// Map from `ChunkId` to its row index in `manifest.chunk_fetcher_rb()`.
    /// Built once at construction; lookups are O(1).
    chunk_id_to_row: HashMap<ChunkId, usize>,
}

impl SegmentChunkProvider {
    /// Fetch the segment manifest from the server and build a provider.
    pub async fn try_new(
        connection_registry: ConnectionRegistryHandle,
        origin: Origin,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> Result<Self, ApiError> {
        let mut client = connection_registry.client(origin.clone()).await?;
        let raw_manifest = client
            .get_rrd_manifest(dataset_id, segment_id.clone())
            .await?;
        let raw_manifest = Arc::new(raw_manifest);

        let manifest = Arc::new(RrdManifest::try_new(&raw_manifest).map_err(|err| {
            ApiError::deserialization_with_source(
                None,
                err,
                "failed to validate RrdManifest from /GetRrdManifest",
            )
        })?);

        let chunk_id_to_row = manifest
            .col_chunk_ids()
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        Ok(Self {
            connection_registry,
            origin,
            dataset_id,
            segment_id,
            raw_manifest,
            manifest,
            chunk_id_to_row,
        })
    }

    pub fn dataset_id(&self) -> EntryId {
        self.dataset_id
    }

    pub fn segment_id(&self) -> &SegmentId {
        &self.segment_id
    }
}

#[async_trait::async_trait]
impl ChunkProvider for SegmentChunkProvider {
    fn manifest(&self) -> &Arc<RrdManifest> {
        &self.manifest
    }

    fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        &self.raw_manifest
    }

    fn source(&self) -> String {
        format!("segment '{}'", self.segment_id)
    }

    async fn load_chunks(&self, ids: &[ChunkId]) -> Result<Vec<Arc<Chunk>>, ChunkProviderError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut row_indices = Vec::with_capacity(ids.len());
        for id in ids {
            let idx = self
                .chunk_id_to_row
                .get(id)
                .copied()
                .ok_or(SegmentProviderError::UnknownChunkId(*id))?;
            row_indices.push(idx);
        }

        let rb = re_arrow_util::take_record_batch(self.manifest.chunk_fetcher_rb(), &row_indices)
            .map_err(SegmentProviderError::Arrow)?;

        let mut client = self
            .connection_registry
            .client(self.origin.clone())
            .await
            .map_err(SegmentProviderError::Api)?;
        let response = client
            .fetch_segment_chunks_by_id(&rb)
            .await
            .map_err(SegmentProviderError::Api)?;
        let mut stream = fetch_chunks_response_to_chunk_and_segment_id(response);

        let mut out = Vec::with_capacity(ids.len());
        while let Some(batch) = stream.next().await {
            for (chunk, _seg_id) in batch.map_err(SegmentProviderError::Api)? {
                out.push(Arc::new(chunk));
            }
        }
        Ok(out)
    }
}

#[derive(Debug, thiserror::Error)]
enum SegmentProviderError {
    #[error("unknown chunk id {0}")]
    UnknownChunkId(ChunkId),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Api(#[from] ApiError),
}

impl From<SegmentProviderError> for ChunkProviderError {
    fn from(err: SegmentProviderError) -> Self {
        Self(Box::new(err))
    }
}
