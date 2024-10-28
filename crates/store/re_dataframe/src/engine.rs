use re_chunk::{EntityPath, TransportChunk};
use re_chunk_store::{ChunkStoreHandle, ColumnDescriptor, QueryExpression};
use re_log_types::EntityPathFilter;
use re_query::QueryCacheHandle;

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
pub struct QueryEngine {
    pub store: ChunkStoreHandle,
    pub cache: QueryCacheHandle,
}

impl QueryEngine {
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
        self.store.read().schema()
    }

    /// Returns the filtered schema for the given [`QueryExpression`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema_for_query(&self, query: &QueryExpression) -> Vec<ColumnDescriptor> {
        self.store.read().schema_for_query(query)
    }

    /// Starts a new query by instantiating a [`QueryHandle`].
    #[inline]
    pub fn query(&self, query: QueryExpression) -> QueryHandle {
        QueryHandle::new(self.clone(), query)
    }

    /// Returns an iterator over all the [`EntityPath`]s present in the database.
    #[inline]
    pub fn iter_entity_paths<'a>(
        &self,
        filter: &'a EntityPathFilter,
    ) -> impl Iterator<Item = EntityPath> + 'a {
        self.store
            .read()
            .all_entities()
            .into_iter()
            .filter(|entity_path| filter.matches(entity_path))
    }
}
