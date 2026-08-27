use std::sync::Arc;

use ahash::HashMap;
use tokio_stream::StreamExt as _;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::{ChunkProvider, ChunkProviderError, RawRrdManifest, RrdManifest};
use re_log_types::EntryId;
use re_types_core::SegmentId;

use crate::asset::{AssetSegments, asset_manifest, asset_segments};
use crate::{
    ApiError, ConnectionClient, ConnectionHandle, fetch_chunks_response_to_chunk_and_segment_id,
};

/// gRPC-backed [`ChunkProvider`]: serves the manifest of a dataset segment
/// and fetches its chunks on demand via `FetchChunks`.
//TODO(RR-4546): this needs to be on par with the table provider stuff in terms of chunk downloading
// (signed url, batching, etc.). The current streaming strategy is really poor, and only works
// because of a workaround we have to mitigate RR-4545
pub struct SegmentChunkProvider {
    connection: ConnectionHandle,
    dataset_id: EntryId,
    segment_id: SegmentId,

    /// The raw manifest of the segment itself, without the assets it references.
    raw_manifest: Arc<RawRrdManifest>,

    /// Every chunk this provider serves: the manifest of the segment, followed by one per asset it
    /// references when `include_assets` was set.
    manifest: Arc<RrdManifest>,

    /// Map from `ChunkId` to its row index in `manifest.chunk_fetcher_rb()`.
    /// Built once at construction; lookups are O(1).
    chunk_id_to_row: HashMap<ChunkId, usize>,
}

impl SegmentChunkProvider {
    /// Fetch the segment manifest from the server and build a provider.
    ///
    /// With `include_assets`, the manifests of the assets the dataset registered are fetched too,
    /// and this provider then also serves their chunks.
    pub async fn try_new(
        connection: ConnectionHandle,
        dataset_id: EntryId,
        segment_id: SegmentId,
        include_assets: bool,
    ) -> Result<Self, ApiError> {
        let mut client = connection.client().await?;
        let raw_manifest = client
            .get_rrd_manifest(dataset_id, segment_id.clone())
            .await?;
        let raw_manifest = Arc::new(raw_manifest);

        let segment_manifest = Arc::new(RrdManifest::try_new(&raw_manifest).map_err(|err| {
            ApiError::deserialization_with_source(
                &connection.origin().clone(),
                None,
                err,
                "failed to validate RrdManifest from /GetRrdManifest",
            )
        })?);

        let asset_manifests = if include_assets {
            fetch_asset_manifests(&mut client, dataset_id).await
        } else {
            Vec::new()
        };

        let manifests: Vec<Arc<RrdManifest>> =
            std::iter::chain(std::iter::once(segment_manifest), asset_manifests).collect();

        // A `FetchChunks` request can span datasets, so one manifest covering the segment and its
        // assets lets any of their chunks be fetched in a single request.
        let manifest = if let [only] = manifests.as_slice() {
            Arc::clone(only)
        } else {
            let parts: Vec<&RrdManifest> = manifests.iter().map(Arc::as_ref).collect();

            Arc::new(RrdManifest::merge(&parts).map_err(|err| {
                ApiError::deserialization_with_source(
                    connection.origin(),
                    None,
                    err,
                    "failed combining the segment and asset manifests",
                )
            })?)
        };

        let chunk_id_to_row = manifest
            .col_chunk_ids()
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        Ok(Self {
            connection,
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

/// Fetches the manifest of every asset registered for the dataset.
///
/// Failing to load an asset only costs the data of that asset, so the failure is logged and the
/// remaining assets are still loaded.
async fn fetch_asset_manifests(
    client: &mut ConnectionClient,
    dataset_id: EntryId,
) -> Vec<Arc<RrdManifest>> {
    let Some(AssetSegments {
        dataset_id: asset_dataset_id,
        segment_ids: asset_segment_ids,
    }) = asset_segments(client, dataset_id).await
    else {
        return Vec::new();
    };

    let mut manifests = Vec::with_capacity(asset_segment_ids.len());
    for asset_segment_id in asset_segment_ids {
        let raw_manifest = match client
            .get_rrd_manifest(asset_dataset_id, asset_segment_id.clone())
            .await
        {
            Ok(raw_manifest) => raw_manifest,
            Err(err) => {
                re_log::warn!(
                    "Failed to fetch asset manifest, skipping it: {err}\nAsset segment id: {asset_segment_id}"
                );
                continue;
            }
        };

        match asset_manifest(client, raw_manifest) {
            Ok((_raw_manifest, manifest)) => manifests.push(Arc::new(manifest)),
            Err(err) => {
                re_log::warn!(
                    "Invalid asset manifest, skipping it: {err}\nAsset segment id: {asset_segment_id}"
                );
            }
        }
    }

    manifests
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
            .connection
            .client()
            .await
            .map_err(SegmentProviderError::Api)?;
        let response = client
            .fetch_segment_chunks_by_id(&rb)
            .await
            .map_err(SegmentProviderError::Api)?;
        let mut stream = fetch_chunks_response_to_chunk_and_segment_id(response, None);

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
