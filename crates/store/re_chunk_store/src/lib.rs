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

mod compact;
mod dataframe;

mod drop_time_range;
pub mod entity_tree;
mod events;
mod gc;
#[cfg(not(target_arch = "wasm32"))]
mod lazy_rrd_store;
mod lineage;
mod missing_chunk_reporter;
mod properties;
mod query;
mod rebatch_videos;
mod split_thick_thin;
mod stats;
mod store;
mod store_schema;
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

pub use self::compact::{CompactionOptions, IsStartOfGop};
pub use self::dataframe::{
    Index, IndexRange, IndexValue, QueryExpression, SparseFillStrategy, StaticColumnSelection,
    ViewContentsSelector,
};
pub use self::entity_tree::EntityTree;
pub use self::events::{
    ChunkComponentMeta, ChunkDeletionReason, ChunkMeta, ChunkStoreDiff, ChunkStoreDiffAddition,
    ChunkStoreDiffDeletion, ChunkStoreDiffSchemaAddition, ChunkStoreDiffVirtualAddition,
    ChunkStoreEvent,
};
pub use self::gc::{GarbageCollectionOptions, GarbageCollectionTarget};
pub use self::lineage::{ChunkDirectLineage, ChunkDirectLineageReport};
pub use self::missing_chunk_reporter::MissingChunkReporter;
pub use self::properties::ExtractPropertiesError;
pub use self::query::QueryResults;
pub use self::stats::{ChunkStoreChunkStats, ChunkStoreStats};
pub use self::store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreHandle, ChunkStoreHandleWeak,
    ColumnMetadata, QueriedChunkIdTracker,
};
pub use self::store_schema::StoreSchema;
pub use self::subscribers::{
    ChunkStoreSubscriber, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber,
};

#[cfg(not(target_arch = "wasm32"))]
pub use self::lazy_rrd_store::LazyRrdStore;

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
pub enum ChunkTrackingMode {
    /// Ignore missing & used chunks, and return partial results.
    Ignore,

    /// Remember the missing & used chunk ID in [`ChunkStore::take_tracked_chunk_ids`].
    Report,

    /// Panic when a chunk is missing.
    ///
    /// Only use this in tests, or contexts where there really can't be
    /// any virtual chunks, and you rather panic than have silent bugs.
    PanicOnMissing,
}
