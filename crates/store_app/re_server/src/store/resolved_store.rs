use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use arrow::array::RecordBatch;
use nohash_hasher::IntSet;
use re_chunk_index::RawRrdManifest;
use re_chunk_store::{
    ChunkStoreHandle, ChunkStoreHandleWeak, ChunkTrackingMode, LazyStore, QueryResults, StoreSchema,
};
use re_log_encoding::{RrdChunkProvider, RrdFooter};
#[cfg(not(target_arch = "wasm32"))]
use re_log_types::StoreKind;
use re_log_types::{EntityPath, StoreId};

/// A store backend: either an in-memory eager store or a provider-backed lazy store.
///
/// Both variants are `Arc`-based, so `Clone` is cheap.
#[derive(Clone)]
pub enum ResolvedStore {
    /// Fully in-memory store for an RRD without a footer.
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

    pub async fn extract_properties(&self) -> Result<RecordBatch, super::Error> {
        match self {
            Self::Eager(h) => h.read().extract_properties(),
            Self::Lazy(l) => l.extract_properties().await,
        }
        .map_err(super::Error::failed_to_extract_properties)
    }

    pub(crate) fn downgrade(&self) -> ResolvedStoreWeak {
        match self {
            Self::Eager(h) => ResolvedStoreWeak::Eager(h.downgrade()),
            Self::Lazy(l) => ResolvedStoreWeak::Lazy(Arc::downgrade(l)),
        }
    }

    /// Load every store of kind `store_kind` in an RRD file, as one [`ResolvedStore`] each.
    ///
    /// Uses the lazy path (chunks loaded on demand) when the RRD has a footer and eager loading
    /// (whole file read into memory) when the footer is missing.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn load_rrd_file(
        path: &Path,
        store_kind: StoreKind,
    ) -> Result<Vec<(StoreId, Self)>, super::Error> {
        if let Some(footer) = read_footer(path).await? {
            let mut stores = Vec::with_capacity(footer.manifests.len());
            for (store_id, raw_manifest) in footer.manifests {
                if store_id.kind() == store_kind {
                    stores.push((store_id, Self::lazy_from_rrd(path, raw_manifest).await?));
                }
            }
            return Ok(stores);
        }

        Ok(load_eager_stores(path)
            .await?
            .into_iter()
            .filter(|(store_id, _)| store_id.kind() == store_kind)
            .map(|(store_id, handle)| (store_id, Self::Eager(handle)))
            .collect())
    }

    /// Load one store of an RRD file, or `None` if the file does not hold `store_id`.
    ///
    /// Lazy or eager under the same rule as [`Self::load_rrd_file`].
    pub async fn load_rrd_store(
        path: &Path,
        store_id: &StoreId,
    ) -> Result<Option<Self>, super::Error> {
        if let Some(mut footer) = read_footer(path).await? {
            return match footer.manifests.remove(store_id) {
                Some(raw_manifest) => Ok(Some(Self::lazy_from_rrd(path, raw_manifest).await?)),
                None => Ok(None),
            };
        }

        Ok(load_eager_stores(path)
            .await?
            .remove(store_id)
            .map(Self::Eager))
    }

    /// A store that reads its chunks on demand from the RRD file at `path`.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        expect(
            clippy::unused_async,
            reason = "opening the file is only asynchronous on wasm"
        )
    )]
    async fn lazy_from_rrd(
        path: &Path,
        raw_manifest: RawRrdManifest,
    ) -> Result<Self, super::Error> {
        let store_file = cfg_select! {
            target_arch = "wasm32" => re_web::fs::File::open(path).await?,
            _ => std::fs::File::open(path)?,
        };

        let provider = Arc::new(
            RrdChunkProvider::from_reader(
                store_file,
                path.display().to_string(),
                Arc::new(raw_manifest),
            )
            .map_err(|err| super::Error::RrdLoadingError(err.into()))?,
        );

        Ok(Self::Lazy(Arc::new(LazyStore::new(provider))))
    }
}

/// The footer of the RRD file at `path`, or `None` if the file was written without one.
async fn read_footer(path: &Path) -> Result<Option<RrdFooter>, super::Error> {
    let file = cfg_select! {
        target_arch = "wasm32" => re_web::fs::File::open(path).await?,
        _ => {
            // TODO(tokio-rs/tokio#1529): Positional reads block the reactor; use `std::fs::File`
            // until an async positional file API lands (or push reads to `spawn_blocking`).
            std::fs::File::open(path)?
        }
    };

    re_log_encoding::read_rrd_footer(&file)
        .await
        .map_err(|err| super::Error::RrdLoadingError(err.into()))
}

/// Every store of the RRD file at `path`, read whole into memory.
async fn load_eager_stores(
    path: &Path,
) -> Result<BTreeMap<StoreId, ChunkStoreHandle>, super::Error> {
    cfg_select! {
        target_arch = "wasm32" => {
            let reader = futures::io::Cursor::new(re_web::fs::read(path).await?);
        }
        _ => {
            use tokio_util::compat::TokioAsyncReadCompatExt as _;
            let reader = tokio::fs::File::open(path).await?.compat();
        }
    }

    re_chunk_store::ChunkStore::handle_from_rrd_reader_async(
        &super::InMemoryStore::default_eager_chunk_store_config(),
        reader,
    )
    .await
    .map_err(super::Error::RrdLoadingError)
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
    use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{EntityPath, StoreId, StoreKind};

    use super::ResolvedStore;

    /// Authors a minimal RRD (one `SetStoreInfo` + a few chunks per store) at `path`, with or
    /// without a footer.
    fn write_rrd(path: &std::path::Path, store_ids: &[StoreId], with_footer: bool) {
        let entity_path = EntityPath::from("/test/entity");
        let timeline = Timeline::new_sequence("frame");

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

        for store_id in store_ids {
            let chunks: Vec<Arc<Chunk>> = (0..3)
                .map(|i| {
                    let points = MyPoint::from_iter((i as u32)..=(i as u32));
                    Arc::new(
                        Chunk::builder(entity_path.clone())
                            .with_sparse_component_batches(
                                RowId::new(),
                                TimePoint::STATIC.with(timeline, i64::from(i)),
                                [(MyPoints::descriptor_points(), Some(&points as _))],
                            )
                            .build()
                            .expect("test chunk should be valid"),
                    )
                })
                .collect();

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
        }

        encoder.finish().expect("failed to finish test RRD");
    }

    /// The register VALIDATION phase enumerates store IDs via
    /// [`re_log_encoding::enumerate_rrd_stores`], while the LOAD phase derives them from
    /// [`ResolvedStore::load_rrd_file`]. These run different code (footer-keys vs lazy load, and
    /// frame-scan vs eager decode for legacy RRDs) and MUST agree: registration loads the stores
    /// that validation enumerated, so it fails on one that loading does not find, and leaves out
    /// one that validation missed. This pins that invariant for both the modern (footer) and
    /// legacy (no-footer) representations.
    #[tokio::test]
    async fn enumerate_and_load_agree_on_store_ids() {
        for with_footer in [true, false] {
            let file = tempfile::NamedTempFile::new().expect("failed to create temp RRD file");
            let path = file.path();
            let store_id = StoreId::random(StoreKind::Recording, "test");
            write_rrd(path, std::slice::from_ref(&store_id), with_footer);

            let file = std::fs::File::open(path).expect("failed to open test RRD file");
            let validated: BTreeSet<StoreId> = re_log_encoding::enumerate_rrd_stores(&file)
                .await
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

    /// Out of a file holding several stores, `load_rrd_store` loads the requested one, and nothing
    /// for a store the file does not hold, with or without a footer.
    #[tokio::test]
    async fn load_rrd_store_loads_the_requested_store() {
        for with_footer in [true, false] {
            let file = tempfile::NamedTempFile::new().expect("failed to create temp RRD file");
            let path = file.path();
            let requested = StoreId::random(StoreKind::Recording, "test");
            let other = StoreId::random(StoreKind::Recording, "test");
            let absent = StoreId::random(StoreKind::Recording, "test");
            write_rrd(path, &[requested.clone(), other], with_footer);

            let loaded = ResolvedStore::load_rrd_store(path, &requested)
                .await
                .expect("failed to load test RRD file")
                .map(|store| store.store_id());
            assert_eq!(loaded, Some(requested), "with_footer={with_footer}");

            let missing = ResolvedStore::load_rrd_store(path, &absent)
                .await
                .expect("failed to load test RRD file")
                .map(|store| store.store_id());
            assert_eq!(missing, None, "with_footer={with_footer}");
        }
    }
}
