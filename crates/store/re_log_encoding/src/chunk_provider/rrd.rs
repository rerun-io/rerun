use std::sync::Arc;

use re_chunk::{Chunk, ChunkId};

use crate::{
    AsyncReadAt, ChunkProvider, ChunkProviderError, CodecResult, RawRrdManifest, RrdManifest,
};

/// Reader-backed [`ChunkProvider`].
pub struct RrdChunkProvider<R: AsyncReadAt> {
    // TODO(grtlr): Change this to be `Mutex` free, but one step at a time.
    reader: futures::lock::Mutex<R>,
    manifest: Arc<RrdManifest>,
    raw_manifest: Arc<RawRrdManifest>,

    /// Human-readable source identifier for diagnostics.
    source: String,
}

impl<R: AsyncReadAt> RrdChunkProvider<R> {
    /// Build a chunk provider from an already-open reader.
    ///
    /// The reader's cursor position is irrelevant; chunk reads seek to absolute manifest offsets.
    pub fn from_reader(
        reader: R,
        source: impl Into<String>,
        raw_manifest: Arc<RawRrdManifest>,
    ) -> CodecResult<Self> {
        let manifest = Arc::new(RrdManifest::try_new(&raw_manifest)?);
        Ok(Self {
            reader: futures::lock::Mutex::new(reader),
            manifest,
            raw_manifest,
            source: source.into(),
        })
    }
}

#[async_trait::async_trait]
impl<R: AsyncReadAt + Send> ChunkProvider for RrdChunkProvider<R> {
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
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut reader = self.reader.lock().await;
        crate::read_chunks(&mut *reader, &self.manifest, ids)
            .await
            .map_err(|err| ChunkProviderError(Box::new(err)))
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use std::path::Path;

    use re_chunk::{RowId, TimePoint, Timeline};
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
        example_components::{MyPoint, MyPoints},
    };

    use crate::EncodingOptions;

    use super::*;

    #[test]
    fn test_chunk_provider_is_dyn_compatible() {
        // Compile-time guard: `ChunkProvider` is dyn-safe and `Arc<RrdChunkProvider<_>>` unsizes
        // to `Arc<dyn ChunkProvider>`.
        fn _assert_object_safe(_: &dyn ChunkProvider) {}

        fn _assert_arc_dyn_constructs(
            p: Arc<RrdChunkProvider<futures::io::AllowStdIo<std::fs::File>>>,
        ) -> Arc<dyn ChunkProvider> {
            p
        }
    }

    /// Build a small RRD file containing `num_chunks` chunks at `path`. Returns `(store_id, chunks)`.
    fn write_test_rrd(path: &Path, num_chunks: usize) -> (StoreId, Vec<Arc<Chunk>>) {
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let store_info = StoreInfo::new(store_id.clone(), StoreSource::Unknown);
        let timeline = Timeline::new_sequence("frame");

        let mut chunks = Vec::with_capacity(num_chunks);
        for frame_idx in 0..num_chunks {
            let entity_path = EntityPath::from(format!("/entity_{frame_idx}"));
            let row_id = RowId::new();
            let points = MyPoint::from_iter(frame_idx as u32..frame_idx as u32 + 1);
            let chunk = Chunk::builder(entity_path)
                .with_sparse_component_batches(
                    row_id,
                    #[expect(clippy::cast_possible_wrap)]
                    TimePoint::default().with(timeline, frame_idx as i64),
                    [(MyPoints::descriptor_points(), Some(&points as _))],
                )
                .build()
                .unwrap();
            chunks.push(Arc::new(chunk));
        }

        let mut file = std::fs::File::create(path).unwrap();
        let mut encoder = crate::Encoder::new_eager(
            crate::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .unwrap();
        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: store_info,
            }))
            .unwrap();
        for chunk in &chunks {
            let arrow_msg = chunk.to_arrow_msg().unwrap();
            encoder
                .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
                .unwrap();
        }
        encoder.finish().unwrap();

        (store_id, chunks)
    }

    #[test]
    fn test_rrd_chunk_provider_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rrd");
        let (store_id, chunks) = write_test_rrd(&path, 3);

        let mut footer_file = futures::io::AllowStdIo::new(std::fs::File::open(&path).unwrap());
        let footer = futures::executor::block_on(crate::read_rrd_footer(&mut footer_file))
            .unwrap()
            .unwrap();
        let raw = Arc::new(footer.manifests[&store_id].clone());
        drop(footer_file);

        let store_file = futures::io::AllowStdIo::new(std::fs::File::open(&path).unwrap());
        let provider =
            RrdChunkProvider::from_reader(store_file, path.display().to_string(), raw).unwrap();

        assert_eq!(provider.manifest().col_chunk_ids().len(), chunks.len());

        let ids: Vec<ChunkId> = provider.manifest().col_chunk_ids().to_vec();
        let loaded = futures::executor::block_on(provider.load_chunks(&ids)).unwrap();

        let mut loaded_ids: Vec<_> = loaded.iter().map(|c| c.id()).collect();
        let mut expected_ids: Vec<_> = ids.clone();
        loaded_ids.sort();
        expected_ids.sort();
        assert_eq!(loaded_ids, expected_ids);
    }
}
