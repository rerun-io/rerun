use std::path::Path;
use std::sync::Arc;

use arrow::array::RecordBatch;
use nohash_hasher::IntSet;
use re_chunk_store::{
    ChunkStoreHandle, ChunkStoreHandleWeak, ChunkTrackingMode, LazyStore, QueryResults, StoreSchema,
};
#[cfg(not(target_arch = "wasm32"))]
use re_log_encoding::RrdChunkProvider;
use re_log_encoding::RrdManifest;
use re_log_types::{EntityPath, StoreId, StoreKind};

/// A store backend: either an in-memory eager store or a provider-backed lazy store.
///
/// Both variants are `Arc`-based, so `Clone` is cheap.
#[derive(Clone)]
pub enum ResolvedStore {
    /// Fully in-memory store (e.g. from `write_chunks` or legacy RRD without footer).
    Eager(ChunkStoreHandle),

    /// Provider-backed store with on-demand chunk loading.
    Lazy(Arc<LazyStore>),
}

impl ResolvedStore {
    pub fn store_id(&self) -> StoreId {
        match self {
            Self::Eager(h) => h.read().id().clone(),
            Self::Lazy(l) => l.store_id().clone(),
        }
    }

    pub fn schema(&self) -> StoreSchema {
        match self {
            Self::Eager(h) => h.read().schema().clone(),
            Self::Lazy(l) => l.schema(),
        }
    }

    pub fn all_entities(&self) -> IntSet<EntityPath> {
        match self {
            Self::Eager(h) => h.read().all_entities(),
            Self::Lazy(l) => l.all_entities(),
        }
    }

    pub fn latest_at_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        match self {
            Self::Eager(h) => h.read().latest_at_relevant_chunks_for_all_components(
                report_mode,
                query,
                entity_path,
                include_static,
            ),
            Self::Lazy(l) => l.latest_at_relevant_chunks_for_all_components(
                report_mode,
                query,
                entity_path,
                include_static,
            ),
        }
    }

    pub fn range_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &re_chunk_store::RangeQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        match self {
            Self::Eager(h) => h.read().range_relevant_chunks_for_all_components(
                report_mode,
                query,
                entity_path,
                include_static,
            ),
            Self::Lazy(l) => l.range_relevant_chunks_for_all_components(
                report_mode,
                query,
                entity_path,
                include_static,
            ),
        }
    }

    pub fn manifest(&self) -> Option<&Arc<RrdManifest>> {
        match self {
            Self::Eager(_) => None,
            Self::Lazy(l) => Some(l.manifest()),
        }
    }

    pub fn extract_properties(&self) -> Result<RecordBatch, super::Error> {
        match self {
            Self::Eager(h) => h.read().extract_properties(),
            Self::Lazy(l) => l.extract_properties(),
        }
        .map_err(super::Error::failed_to_extract_properties)
    }

    pub(crate) fn downgrade(&self) -> ResolvedStoreWeak {
        match self {
            Self::Eager(h) => ResolvedStoreWeak::Eager(h.downgrade()),
            Self::Lazy(l) => ResolvedStoreWeak::Lazy(Arc::downgrade(l)),
        }
    }

    /// Load an RRD reader as one or more _eager_ [`ResolvedStore`]s, one per store found in the stream.
    ///
    /// Stores whose kind does not match `store_kind` are filtered out.
    fn load_rrd_reader_eager(
        reader: impl std::io::Read,
        store_kind: StoreKind,
        config: &re_chunk_store::ChunkStoreConfig,
    ) -> Result<Vec<(StoreId, Self)>, super::Error> {
        Ok(
            re_chunk_store::ChunkStore::handle_from_rrd_reader(config, reader)
                .map_err(super::Error::RrdLoadingError)?
                .into_iter()
                .filter(|(store_id, _)| store_id.kind() == store_kind)
                .map(|(store_id, handle)| (store_id, Self::Eager(handle)))
                .collect(),
        )
    }

    /// Load an RRD file as one or more [`ResolvedStore`]s, one per store found in the file.
    ///
    /// Prefers the lazy path (chunks loaded on demand) when the RRD has a footer; falls back to
    /// eager loading (whole file read into memory) when the footer is missing or unreadable.
    /// Stores whose kind does not match `store_kind` are filtered out.
    pub async fn load_rrd_file(
        path: &Path,
        store_kind: StoreKind,
    ) -> Result<Vec<(StoreId, Self)>, super::Error> {
        #[cfg(target_arch = "wasm32")]
        {
            let bytes = crate::opfs::read(path).await?;

            // TODO(RR-5086): Ultimately, we want to be able to load from an OPFS file into a lazy store too.
            Self::load_rrd_reader_eager(
                std::io::Cursor::new(bytes),
                store_kind,
                &super::InMemoryStore::default_eager_chunk_store_config(),
            )
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut file = tokio::fs::File::open(path).await?.into_std().await;

            if let Ok(Some(footer)) = re_log_encoding::read_rrd_footer(&mut file) {
                // The footer-reading handle is no longer needed — each `LazyStore` holds its own.
                drop(file);

                let mut out = Vec::with_capacity(footer.manifests.len());
                for (store_id, raw_manifest) in footer.manifests {
                    if store_id.kind() != store_kind {
                        continue;
                    }
                    let store_file = tokio::fs::File::open(path).await?.into_std().await;
                    let provider = Arc::new(
                        RrdChunkProvider::try_from_file(store_file, path, Arc::new(raw_manifest))
                            .map_err(|err| super::Error::RrdLoadingError(err.into()))?,
                    );
                    let lazy = Arc::new(LazyStore::new(provider));
                    out.push((store_id, Self::Lazy(lazy)));
                }
                Ok(out)
            } else {
                // Legacy fallback: eager load (no footer, or footer read error).
                use std::io::Seek as _;
                file.seek(std::io::SeekFrom::Start(0))?;
                Self::load_rrd_reader_eager(
                    file,
                    store_kind,
                    &super::InMemoryStore::default_eager_chunk_store_config(),
                )
            }
        }
    }
}

/// Weak counterpart of [`ResolvedStore`], held by [`StorePool`](super::store_pool::StorePool).
pub(crate) enum ResolvedStoreWeak {
    Eager(ChunkStoreHandleWeak),
    Lazy(std::sync::Weak<LazyStore>),
}

impl ResolvedStoreWeak {
    pub fn upgrade(&self) -> Option<ResolvedStore> {
        match self {
            Self::Eager(w) => w.upgrade().map(ResolvedStore::Eager),
            Self::Lazy(w) => w.upgrade().map(ResolvedStore::Lazy),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, TimePoint, Timeline};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
    };

    use super::ResolvedStore;

    /// Authors a minimal RRD (one `SetStoreInfo` + a few chunks) at `path`, with or without a
    /// footer, and returns the `StoreId` that was written.
    fn write_rrd(path: &std::path::Path, store_id: &StoreId, with_footer: bool) {
        let entity_path = EntityPath::from("/test/entity");
        let timeline = Timeline::new_sequence("frame");
        let chunks: Vec<Arc<Chunk>> = (0..3)
            .map(|i| {
                let points = MyPoint::from_iter(i as u32..i as u32 + 1);
                Arc::new(
                    Chunk::builder(entity_path.clone())
                        .with_sparse_component_batches(
                            RowId::new(),
                            TimePoint::default().with(timeline, i64::from(i)),
                            [(MyPoints::descriptor_points(), Some(&points as _))],
                        )
                        .build()
                        .expect("test chunk should be valid"),
                )
            })
            .collect();

        let mut file = std::fs::File::create(path).expect("failed to create test RRD file");
        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .expect("failed to create test RRD encoder");
        if !with_footer {
            encoder.do_not_emit_footer();
        }
        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
            }))
            .expect("failed to write test store info");
        for chunk in &chunks {
            encoder
                .append(&LogMsg::ArrowMsg(
                    store_id.clone(),
                    chunk
                        .to_arrow_msg()
                        .expect("test chunk should encode as arrow"),
                ))
                .expect("failed to write test chunk");
        }
        encoder.finish().expect("failed to finish test RRD");
    }

    /// The register VALIDATION phase enumerates store IDs via
    /// [`re_log_encoding::enumerate_rrd_stores`], while the LOAD phase derives them from
    /// [`ResolvedStore::load_rrd_file`]. These run different code (footer-keys vs lazy load, and
    /// frame-scan vs eager decode for legacy RRDs) and MUST agree, or registration would validate
    /// a different set of segments than it ends up loading. This pins that invariant for both the
    /// modern (footer) and legacy (no-footer) representations.
    #[tokio::test]
    async fn enumerate_and_load_agree_on_store_ids() {
        for with_footer in [true, false] {
            let file = tempfile::NamedTempFile::new().expect("failed to create temp RRD file");
            let path = file.path();
            let store_id = StoreId::random(StoreKind::Recording, "test");
            write_rrd(path, &store_id, with_footer);

            let validated: BTreeSet<StoreId> = re_log_encoding::enumerate_rrd_stores(
                &mut std::fs::File::open(path).expect("failed to open test RRD file"),
            )
            .expect("failed to enumerate test RRD stores")
            .into_iter()
            .filter(|id| id.kind() == StoreKind::Recording)
            .collect();

            let loaded: BTreeSet<StoreId> =
                ResolvedStore::load_rrd_file(path, StoreKind::Recording)
                    .await
                    .expect("failed to load test RRD file")
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();

            assert_eq!(
                validated, loaded,
                "validate/load store-id sets must agree (with_footer={with_footer})"
            );
            assert_eq!(loaded, BTreeSet::from([store_id]));
        }
    }
}
