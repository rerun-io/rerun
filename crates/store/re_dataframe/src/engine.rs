use re_chunk::TransportChunk;
use re_chunk_store::{
    ChunkStore, ColumnDescriptor, LatestAtQueryExpression, QueryExpression, RangeQueryExpression,
};
use re_query::Caches;

use crate::LatestAtQueryHandle;

// ---

// TODO(#3741): `arrow2` has no concept of a `RecordBatch`, so for now we just use our trustworthy
// `TransportChunk` type until we migrate to `arrow-rs`.
// `TransportChunk` maps 1:1 to `RecordBatch` so the switch (and the compatibility layer in the meantime)
// will be trivial.
// TODO(cmc): add an `arrow` feature to transportchunk in a follow-up pr and call it a day.
pub type RecordBatch = TransportChunk;

pub struct RangeQueryHandle<'a>(&'a ());

/// A generic handle to a query that is ready to be executed.
pub enum QueryHandle<'a> {
    LatestAt(LatestAtQueryHandle<'a>),
    Range(RangeQueryHandle<'a>),
}

impl<'a> From<LatestAtQueryHandle<'a>> for QueryHandle<'a> {
    #[inline]
    fn from(query: LatestAtQueryHandle<'a>) -> Self {
        Self::LatestAt(query)
    }
}

impl<'a> From<RangeQueryHandle<'a>> for QueryHandle<'a> {
    #[inline]
    fn from(query: RangeQueryHandle<'a>) -> Self {
        Self::Range(query)
    }
}

// --- Queries ---

/// A handle to our user-facing query engine.
///
/// See the following methods:
/// * [`QueryEngine::schema`]: get the complete schema of the recording.
/// * [`QueryEngine::latest_at`]: get a snapshot of the latest state of a dataset at specific point in time.
/// * [`QueryEngine::range`]: get successive dense snapshots of the latest state of a dataset over
///   a range of time.
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

    /// Returns the filtered schema for the given query expression.
    ///
    /// This will only include columns which may contain non-empty values from the perspective of
    /// the query semantics.
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the control columns in lexical order (`RowId`);
    /// * second, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * third, the component columns in lexical order (`Color`, `Radius, ...`).
    ///
    /// This does not run a full-blown query, but rather just inspects [`Chunk`]-level metadata,
    /// which can lead to false positives, but makes this very cheap to compute.
    #[inline]
    pub fn schema_for_query(&self, query: &QueryExpression) -> Vec<ColumnDescriptor> {
        self.store.schema_for_query(query)
    }

    /// Creates a new appropriate [`QueryHandle`].
    ///
    /// This is simply a helper for:
    /// * [`Self::latest_at`]
    /// * [`Self::range`]
    #[inline]
    #[allow(clippy::unimplemented, clippy::needless_pass_by_value)]
    pub fn query(
        &self,
        query: &QueryExpression,
        columns: Option<Vec<ColumnDescriptor>>,
    ) -> QueryHandle<'_> {
        match query {
            QueryExpression::LatestAt(query) => self.latest_at(query, columns).into(),
            QueryExpression::Range(query) => self.range(query, columns).into(),
        }
    }

    /// Creates a new [`LatestAtQueryHandle`], which can be used to perform a latest-at query.
    ///
    /// Creating a handle is very cheap as it doesn't perform any kind of querying.
    ///
    /// If `columns` is specified, the schema of the result will strictly follow this specification.
    /// Any provided [`ColumnDescriptor`]s that don't match a column in the result will still be included, but the
    /// data will be null for the entire column.
    /// If `columns` is left unspecified, the schema of the returned result will correspond to what's returned by
    /// [`Self::schema_for_query`].
    /// Seel also [`LatestAtQueryHandle::schema`].
    ///
    /// Because data is often logged concurrently across multiple timelines, the non-primary timelines
    /// are still valid data-columns to include in the result. So a user could, for example, query
    /// for a range of data on the `frame` timeline, but still include the `log_time` timeline in
    /// the result.
    #[inline]
    pub fn latest_at(
        &self,
        query: &LatestAtQueryExpression,
        columns: Option<Vec<ColumnDescriptor>>,
    ) -> LatestAtQueryHandle<'_> {
        LatestAtQueryHandle::new(self, query.clone(), columns)
    }

    /// Creates a new [`RangeQueryHandle`], which can be used to perform a range query.
    ///
    /// Creating a handle is very cheap as it doesn't perform any kind of querying.
    ///
    /// If `columns` is specified, the schema of the result will strictly follow this specification.
    /// Any provided [`ColumnDescriptor`]s that don't match a column in the result will still be included, but the
    /// data will be null for the entire column.
    /// If `columns` is left unspecified, the schema of the returned result will correspond to what's returned by
    /// [`Self::schema_for_query`].
    /// Seel also [`RangeQueryHandle::schema`].
    ///
    /// Because data is often logged concurrently across multiple timelines, the non-primary timelines
    /// are still valid data-columns to include in the result. So a user could, for example, query
    /// for a range of data on the `frame` timeline, but still include the `log_time` timeline in
    /// the result.
    #[inline]
    #[allow(clippy::unimplemented, clippy::needless_pass_by_value)]
    pub fn range(
        &self,
        query: &RangeQueryExpression,
        columns: Option<Vec<ColumnDescriptor>>,
    ) -> RangeQueryHandle<'_> {
        _ = self;
        _ = query;
        _ = columns;
        unimplemented!("TODO(cmc)")
    }
}
