//! The Rerun chunk store, implemented on top of [Apache Arrow](https://arrow.apache.org/)
//! using the [`arrow`] crate.
//!
//! This crate is an in-memory time series database for Rerun log data.
//! It is indexed by Entity path, component, timeline, and time.
//! It supports out-of-order insertions, and fast `O(log(N))` queries.
//!
//! * See [`ChunkStore`] for an overview of the core data structures.
//! * See [`ChunkStore::latest_at_relevant_chunks`] and [`ChunkStore::range_relevant_chunks`]
//!   for the documentation of the public read APIs.
//! * See [`ChunkStore::insert_chunk`] for the documentation of the public write APIs.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod dataframe;
mod drop_time_range;
mod events;
mod gc;
mod query;
mod stats;
mod store;
mod subscribers;
mod writes;

mod protobuf_conversions;

pub use self::dataframe::{
    ColumnDescriptor, ColumnSelector, ComponentColumnDescriptor, ComponentColumnSelector, Index,
    IndexRange, IndexValue, QueryExpression, SparseFillStrategy, TimeColumnDescriptor,
    TimeColumnSelector, ViewContentsSelector,
};
pub use self::events::{
    ChunkCompactionReport, ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent,
};
pub use self::gc::{GarbageCollectionOptions, GarbageCollectionTarget};
pub use self::stats::{ChunkStoreChunkStats, ChunkStoreStats};
pub use self::store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreHandle, ColumnMetadata,
};
pub use self::subscribers::{
    ChunkStoreSubscriber, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber,
};

pub(crate) use self::store::ColumnMetadataState;

// Re-exports
#[doc(no_inline)]
pub use re_chunk::{
    Chunk, ChunkId, ChunkShared, LatestAtQuery, RangeQuery, RangeQueryOptions, RowId,
    UnitChunkShared,
};

#[doc(no_inline)]
pub use re_log_types::{ResolvedTimeRange, TimeInt, TimeType, Timeline};

pub mod external {
    pub use arrow;

    pub use re_chunk;
}

// ---

#[derive(thiserror::Error, Debug)]
pub enum ChunkStoreError {
    #[error("Chunks must be sorted before insertion in the chunk store")]
    UnsortedChunk,

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    /// Error when parsing configuration from environment.
    #[error("Failed to parse config: '{name}={value}': {err}")]
    ParseConfig {
        name: &'static str,
        value: String,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type ChunkStoreResult<T> = ::std::result::Result<T, ChunkStoreError>;
