use re_chunk::{EntityPath, TransportChunk};
use re_chunk_store::{ChunkStore, ColumnDescriptor, QueryExpression2, ViewContentsSelector};
use re_log_types::EntityPathFilter;
use re_query::Caches;

use crate::QueryHandle;

// Used all over in docstrings.
#[allow(unused_imports)]
use re_chunk_store::ComponentColumnDescriptor;

// ---

// TODO(#3741): `arrow2` has no concept of a `RecordBatch`, so for now we just use our trustworthy
// `TransportChunk` type until we migrate to `arrow-rs`.
// `TransportChunk` maps 1:1 to `RecordBatch` so the switch (and the compatibility layer in the meantime)
// will be trivial.
// TODO(cmc): add an `arrow` feature to transportchunk in a follow-up pr and call it a day.
pub type RecordBatch = TransportChunk;

// --- Queries ---

/// A handle to our user-facing query engine.
///
/// See the following methods:
/// * [`QueryEngine::schema`]: get the complete schema of the recording.
/// * [`QueryEngine::query`]: execute a [`QueryExpression2`] on the recording.
//
// TODO(cmc): This needs to be a refcounted type that can be easily be passed around: the ref has
// got to go. But for that we need to generally introduce `ChunkStoreHandle` and `QueryCacheHandle`
// first, and this is not as straightforward as it seems.
pub struct QueryEngine<'a> {
    pub store: &'a ChunkStore,
    pub cache: &'a Caches,
}

impl QueryEngine<'_> {
    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns to guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema(&self) -> Vec<ColumnDescriptor> {
        self.store.schema()
    }

    /// Returns the filtered schema for the given `view_contents`.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema_for_view_contents(
        &self,
        view_contents: &ViewContentsSelector,
    ) -> Vec<ColumnDescriptor> {
        self.store.schema_for_view_contents(view_contents)
    }

    /// Starts a new query by instantiating a [`QueryHandle`].
    #[inline]
    pub fn query(&self, query: QueryExpression2) -> QueryHandle<'_> {
        QueryHandle::new(self, query)
    }

    /// Returns an iterator over all the [`EntityPath`]s present in the database.
    #[inline]
    pub fn iter_entity_paths<'a>(
        &self,
        filter: &'a EntityPathFilter,
    ) -> impl Iterator<Item = EntityPath> + 'a {
        self.store
            .all_entities()
            .into_iter()
            .filter(|entity_path| filter.matches(entity_path))
    }
}
