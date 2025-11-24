//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod builder;
mod chunk;
mod helpers;
mod iter;
mod latest_at;
mod merge;
mod range;
mod shuffle;
mod slice;
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
pub use self::{
    builder::{ChunkBuilder, TimeColumnBuilder},
    chunk::{Chunk, ChunkComponents, ChunkError, ChunkResult, TimeColumn, TimeColumnError},
    helpers::{ChunkShared, UnitChunkShared},
    iter::{ChunkComponentIter, ChunkComponentIterItem, ChunkComponentSlicer, ChunkIndicesIter},
    latest_at::LatestAtQuery,
    range::{RangeQuery, RangeQueryOptions},
};

pub mod external {
    pub use arrow;
    #[cfg(not(target_arch = "wasm32"))]
    pub use crossbeam;
    pub use nohash_hasher;
    pub use re_byte_size;
    pub use re_log_types;
}
