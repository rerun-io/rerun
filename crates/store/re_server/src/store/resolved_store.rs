use std::path::Path;
use std::sync::Arc;

use arrow::array::RecordBatch;
use nohash_hasher::IntSet;
use re_chunk_store::{
    Chunk, ChunkId, ChunkStore, ChunkStoreHandle, ChunkStoreHandleWeak, ChunkTrackingMode,
    LazyRrdStore, QueryResults, StoreSchema,
};
use re_log_encoding::RrdManifest;
use re_log_types::{EntityPath, StoreId, StoreKind};

/// A store backend: either an in-memory eager store or a file-backed lazy store.
///
/// Both variants are `Arc`-based, so `Clone` is cheap.
#[derive(Clone)]
pub enum ResolvedStore {
    /// Fully in-memory store (e.g. from `write_chunks` or legacy RRD without footer).
    Eager(ChunkStoreHandle),

    /// File-backed store with on-demand chunk loading.
    Lazy(Arc<LazyRrdStore>),
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

    pub fn physical_chunk(&self, id: &ChunkId) -> Option<Arc<Chunk>> {
        match self {
            Self::Eager(h) => h.read().physical_chunk(id).cloned(),
            Self::Lazy(l) => l.physical_chunk(id),
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

    /// Load an RRD file as one or more [`ResolvedStore`]s, one per store found in the file.
    ///
    /// Prefers the lazy path (chunks loaded on demand) when the RRD has a footer; falls back to
    /// eager loading (whole file read into memory) when the footer is missing or unreadable.
    /// Stores whose kind does not match `store_kind` are filtered out.
    pub fn load_rrd_file(
        path: &Path,
        store_kind: StoreKind,
    ) -> Result<Vec<(StoreId, Self)>, super::Error> {
        let mut file = std::fs::File::open(path)?;

        if let Ok(Some(footer)) = re_log_encoding::read_rrd_footer(&mut file) {
            // The footer-reading handle is no longer needed — each `LazyRrdStore` holds its own.
            drop(file);

            let mut out = Vec::with_capacity(footer.manifests.len());
            for (store_id, raw_manifest) in footer.manifests {
                if store_id.kind() != store_kind {
                    continue;
                }
                let store_file = std::fs::File::open(path)?;
                let lazy = Arc::new(
                    LazyRrdStore::try_new(store_file, path.to_owned(), Arc::new(raw_manifest))
                        .map_err(|err| super::Error::RrdLoadingError(err.into()))?,
                );
                out.push((store_id, Self::Lazy(lazy)));
            }
            Ok(out)
        } else {
            // Legacy fallback: eager load (no footer, or footer read error).
            let contents = ChunkStore::handle_from_rrd_filepath(
                &super::InMemoryStore::chunk_store_config(),
                path,
            )
            .map_err(super::Error::RrdLoadingError)?;

            Ok(contents
                .into_iter()
                .filter(|(store_id, _)| store_id.kind() == store_kind)
                .map(|(store_id, handle)| (store_id, Self::Eager(handle)))
                .collect())
        }
    }
}

/// Weak counterpart of [`ResolvedStore`], held by [`StorePool`](super::store_pool::StorePool).
pub(crate) enum ResolvedStoreWeak {
    Eager(ChunkStoreHandleWeak),
    Lazy(std::sync::Weak<LazyRrdStore>),
}

impl ResolvedStoreWeak {
    pub fn upgrade(&self) -> Option<ResolvedStore> {
        match self {
            Self::Eager(w) => w.upgrade().map(ResolvedStore::Eager),
            Self::Lazy(w) => w.upgrade().map(ResolvedStore::Lazy),
        }
    }
}
