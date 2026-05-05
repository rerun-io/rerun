use std::sync::Arc;

use ahash::HashMap;
use tokio::runtime::Handle;
use tokio_stream::StreamExt as _;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::{ChunkProvider, ChunkProviderError, RawRrdManifest, RrdManifest};
use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_uri::Origin;

use crate::{ApiError, ConnectionRegistryHandle, fetch_chunks_response_to_chunk_and_segment_id};

/// gRPC-backed [`ChunkProvider`]: serves the manifest of a single dataset
/// segment and fetches its chunks on demand via `FetchChunks`.
///
/// # Runtime requirement on `load_chunks`
///
/// [`Self::load_chunks`] performs a synchronous `Handle::block_on`. **It must
/// not be called from a future running on the same tokio runtime as
/// `runtime_handle`** — doing so will deadlock.
///
/// # Worker-thread requirement
///
/// `fetch_chunks_response_to_chunk_and_segment_id` uses
/// `tokio::task::spawn_blocking` internally. Under `block_on`, this requires
/// the runtime to have ≥ 1 free worker thread. The process-wide runtime in
/// the Python bindings is multi-threaded with `num_cpus()` workers, so this
/// is satisfied in practice.
//TODO(RR-4546): this needs to be on par with the table provider stuff in terms of chunk downloading
// (signed url, batching, etc.). The current streaming strategy is really poor, and only works
// because of a workaround we have to mitigate RR-4545
pub struct SegmentChunkProvider {
    runtime_handle: Handle,
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
        runtime_handle: Handle,
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
            runtime_handle,
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

impl ChunkProvider for SegmentChunkProvider {
    fn manifest(&self) -> &Arc<RrdManifest> {
        &self.manifest
    }

    fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        &self.raw_manifest
    }

    fn source(&self) -> String {
        format!("segment '{}'", self.segment_id.id)
    }

    fn load_chunks(&self, ids: &[ChunkId]) -> Result<Vec<Arc<Chunk>>, ChunkProviderError> {
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

        let connection_registry = self.connection_registry.clone();
        let origin = self.origin.clone();

        // SAFETY: see the runtime-requirement note on the type. This deadlocks
        // if `runtime_handle` is the runtime currently driving us.
        self.runtime_handle
            .block_on(async move {
                let mut client = connection_registry.client(origin).await?;
                let response = client.fetch_segment_chunks_by_id(&rb).await?;
                let mut stream = fetch_chunks_response_to_chunk_and_segment_id(response);

                let mut out = Vec::with_capacity(ids.len());
                while let Some(batch) = stream.next().await {
                    for (chunk, _seg_id) in batch? {
                        out.push(Arc::new(chunk));
                    }
                }
                Ok::<_, ApiError>(out)
            })
            .map_err(|err| SegmentProviderError::Api(err).into())
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
