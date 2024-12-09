use std::collections::BTreeMap;

use re_chunk::{EntityPath, TransportChunk};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ColumnDescriptor, QueryExpression,
    VersionPolicy,
};
use re_log_types::{EntityPathFilter, StoreId};
use re_query::{QueryCache, QueryCacheHandle, StorageEngine, StorageEngineLike};

use crate::QueryHandle;

// Used all over in docstrings.
#[allow(unused_imports)]
use re_chunk_store::ComponentColumnDescriptor;

// ---

// TODO(#3741): `arrow2` has no concept of a `RecordBatch`, so for now we just use our trustworthy
// `TransportChunk` type until we migrate to `arrow-rs`.
// `TransportChunk` maps 1:1 to `RecordBatch` so the switch (and the compatibility layer in the meantime)
// will be trivial.
pub type RecordBatch = TransportChunk;

// --- Queries ---

/// A handle to our user-facing query engine.
///
/// Cheap to clone.
///
/// See the following methods:
/// * [`QueryEngine::schema`]: get the complete schema of the recording.
/// * [`QueryEngine::query`]: execute a [`QueryExpression`] on the recording.
#[derive(Clone)]
pub struct QueryEngine<E: StorageEngineLike> {
    pub engine: E,
}

impl QueryEngine<StorageEngine> {
    #[inline]
    pub fn new(store: ChunkStoreHandle, cache: QueryCacheHandle) -> Self {
        // Safety: EntityDb's handles can never be accessed from the outside, therefore these
        // handles had to have been constructed in an external context, outside of the main app.
        #[allow(unsafe_code)]
        let engine = unsafe { StorageEngine::new(store, cache) };

        Self { engine }
    }

    /// This will automatically instantiate a new empty [`QueryCache`].
    #[inline]
    pub fn from_store(store: ChunkStoreHandle) -> Self {
        Self::new(store.clone(), QueryCache::new_handle(store))
    }

    /// Like [`ChunkStore::from_rrd_filepath`], but automatically instantiates [`QueryEngine`]s
    /// with new empty [`QueryCache`]s.
    #[inline]
    pub fn from_rrd_filepath(
        store_config: &ChunkStoreConfig,
        path_to_rrd: impl AsRef<std::path::Path>,
        version_policy: re_log_encoding::VersionPolicy,
    ) -> anyhow::Result<BTreeMap<StoreId, Self>> {
        Ok(
            ChunkStore::handle_from_rrd_filepath(store_config, path_to_rrd, version_policy)?
                .into_iter()
                .map(|(store_id, store)| (store_id, Self::from_store(store)))
                .collect(),
        )
    }
}

impl<E: StorageEngineLike + Clone> QueryEngine<E> {
    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns to guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema(&self) -> Vec<ColumnDescriptor> {
        self.engine.with(|store, _cache| store.schema())
    }

    /// Returns the filtered schema for the given [`QueryExpression`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema_for_query(&self, query: &QueryExpression) -> Vec<ColumnDescriptor> {
        self.engine
            .with(|store, _cache| store.schema_for_query(query))
    }

    /// Starts a new query by instantiating a [`QueryHandle`].
    #[inline]
    pub fn query(&self, query: QueryExpression) -> QueryHandle<E> {
        QueryHandle::new(self.engine.clone(), query)
    }

    /// Returns an iterator over all the [`EntityPath`]s present in the database.
    #[inline]
    pub fn iter_entity_paths_sorted<'a>(
        &self,
        filter: &'a EntityPathFilter,
    ) -> impl Iterator<Item = EntityPath> + 'a {
        self.engine.with(|store, _cache| {
            store
                .all_entities_sorted()
                .into_iter()
                .filter(|entity_path| filter.matches(entity_path))
        })
    }
}
