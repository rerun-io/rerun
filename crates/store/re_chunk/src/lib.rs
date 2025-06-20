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

pub use self::builder::{ChunkBuilder, TimeColumnBuilder};
pub use self::chunk::{
    Chunk, ChunkComponents, ChunkError, ChunkResult, TimeColumn, TimeColumnError,
};
pub use self::helpers::{ChunkShared, UnitChunkShared};
pub use self::iter::{
    ChunkComponentIter, ChunkComponentIterItem, ChunkComponentSlicer, ChunkIndicesIter,
};
pub use self::latest_at::LatestAtQuery;
pub use self::range::{RangeQuery, RangeQueryOptions};

#[cfg(not(target_arch = "wasm32"))]
pub use self::batcher::{
    ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkBatcherResult, PendingRow,
};

// Re-exports

#[doc(no_inline)]
pub use arrow::array::Array as ArrowArray;
#[doc(no_inline)]
pub use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline, TimelineName};
#[doc(no_inline)]
pub use re_types_core::{ArchetypeName, ChunkId, ComponentIdentifier, ComponentType, RowId};

pub mod external {
    pub use arrow;
    pub use nohash_hasher;

    pub use re_byte_size;
    pub use re_log_types;

    #[cfg(not(target_arch = "wasm32"))]
    pub use crossbeam;
}
