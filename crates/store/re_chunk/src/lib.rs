//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod builder;
mod cast;
mod chunk;
mod helpers;
mod iter;
mod latest_at;
mod merge;
mod range;
mod shuffle;
mod slice;
mod split;
mod transport;

#[cfg(not(target_arch = "wasm32"))]
mod batcher;

// Re-exports
#[doc(no_inline)]
pub use {
    arrow::array::Array as ArrowArray,
    re_log_types::{EntityPath, TimeInt, TimePoint, Timeline, TimelineName},
    re_span::Span,
    re_types_core::{ArchetypeName, ChunkId, ComponentIdentifier, ComponentType, RowId},
};

#[cfg(not(target_arch = "wasm32"))]
pub use self::batcher::{
    BatcherFlushError, BatcherHooks, ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError,
    ChunkBatcherResult, PendingRow,
};
pub use self::builder::{ChunkBuilder, TimeColumnBuilder};
pub use self::cast::CastToPrimitive;
pub use self::chunk::{
    Chunk, ChunkComponents, ChunkError, ChunkResult, TimeColumn, TimeColumnError,
};
pub use self::helpers::{ChunkShared, UnitChunkShared};
pub use self::iter::{
    // TODO: Right place? Maybe `re_view`?
    CastToPrimitive,
    ChunkComponentIter,
    ChunkComponentIterItem,
    ChunkComponentSlicer,
    ChunkIndicesIter,
};
pub use self::latest_at::LatestAtQuery;
pub use self::range::{RangeQuery, RangeQueryOptions};
pub use self::split::ChunkSplitConfig;

pub mod external {
    #[cfg(not(target_arch = "wasm32"))]
    pub use crossbeam;
    pub use {arrow, nohash_hasher, re_byte_size, re_log_types};
}
