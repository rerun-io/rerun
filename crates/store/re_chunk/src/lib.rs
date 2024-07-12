//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod builder;
mod chunk;
mod id;
mod iter;
mod latest_at;
mod merge;
mod range;
mod shuffle;
mod slice;
mod transport;
pub mod util;

#[cfg(not(target_arch = "wasm32"))]
mod batcher;

pub use self::builder::{ChunkBuilder, ChunkTimelineBuilder};
pub use self::chunk::{Chunk, ChunkError, ChunkResult, ChunkTimeline};
pub use self::id::{ChunkId, RowId};
pub use self::latest_at::LatestAtQuery;
pub use self::range::RangeQuery;
pub use self::transport::TransportChunk;

#[cfg(not(target_arch = "wasm32"))]
pub use self::batcher::{
    ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkBatcherResult, PendingRow,
};

// Re-exports

#[doc(no_inline)]
pub use arrow2::array::Array as ArrowArray;
#[doc(no_inline)]
pub use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline, TimelineName};
#[doc(no_inline)]
pub use re_types_core::ComponentName;

pub mod external {
    pub use arrow2;

    pub use re_log_types;

    #[cfg(not(target_arch = "wasm32"))]
    pub use crossbeam;
}
