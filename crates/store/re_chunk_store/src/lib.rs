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
mod lineage;
mod properties;
mod query;
mod stats;
mod store;
mod subscribers;
mod writes;

// Re-exports
#[doc(no_inline)]
pub use {
    re_chunk::{
        Chunk, ChunkId, ChunkShared, LatestAtQuery, RangeQuery, RangeQueryOptions, RowId, Span,
        UnitChunkShared,
    },
    re_log_types::{AbsoluteTimeRange, TimeInt, TimeType, Timeline},
    re_sorbet::{ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor},
};

pub use self::dataframe::{
    Index, IndexRange, IndexValue, QueryExpression, SparseFillStrategy, StaticColumnSelection,
    ViewContentsSelector,
};
pub use self::events::{
    ChunkStoreDiff, ChunkStoreDiffAddition, ChunkStoreDiffDeletion, ChunkStoreDiffVirtualAddition,
    ChunkStoreEvent,
};
pub use self::gc::{GarbageCollectionOptions, GarbageCollectionTarget};
pub use self::lineage::{ChunkDirectLineage, ChunkDirectLineageReport};
pub use self::properties::ExtractPropertiesError;
pub use self::query::QueryResults;
pub use self::stats::{ChunkStoreChunkStats, ChunkStoreStats};
pub use self::store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreHandle, ColumnMetadata,
};
pub use self::subscribers::{
    ChunkStoreSubscriber, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber,
};

pub(crate) use self::store::ColumnMetadataState;

pub mod external {
    pub use {arrow, re_chunk};
}

// ---

#[derive(thiserror::Error, Debug)]
pub enum ChunkStoreError {
    #[error("Chunks must be sorted before insertion in the chunk store")]
    UnsortedChunk,

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    #[error("Failed to load data, parsing error: {0:#}")]
    Codec(#[from] re_log_encoding::CodecError),

    #[error("Failed to load data, semantic error: {0:#}")]
    Sorbet(#[from] re_sorbet::SorbetError),

    /// Error when parsing configuration from environment.
    #[error("Failed to parse config: '{name}={value}': {err}")]
    ParseConfig {
        name: &'static str,
        value: String,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type ChunkStoreResult<T> = ::std::result::Result<T, ChunkStoreError>;

/// What to do when a virtual chunk is missing from the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnMissingChunk {
    /// Ignore the missing chunk, and return partial results.
    Ignore,

    /// Remember the missing chunk ID in [`ChunkStore::take_missing_chunk_ids`]
    /// and report it back in [`QueryResults::missing`].
    Report,

    /// Panic when a chunk is missing.
    ///
    /// Only use this in tests!
    Panic,
}
