//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod builder;
mod chunk;
mod latest_at;
mod range;
mod shuffle;
mod slice;
mod transport;
pub mod util;

#[cfg(not(target_arch = "wasm32"))]
mod batcher;

pub use self::chunk::{Chunk, ChunkError, ChunkId, ChunkResult, ChunkTimeline};
pub use self::latest_at::LatestAtQuery;
pub use self::range::RangeQuery;
pub use self::transport::TransportChunk;

#[cfg(not(target_arch = "wasm32"))]
pub use self::batcher::{
    ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkBatcherResult, PendingRow,
};

// Re-exports
pub use re_log_types::{EntityPath, RowId, TimeInt, TimePoint, Timeline, TimelineName};
pub use re_types_core::ComponentName;

pub mod external {
    pub use arrow2;
    pub use crossbeam;

    pub use re_log_types;
}
