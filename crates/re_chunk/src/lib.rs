//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod chunk;
mod shuffle;
mod transport;
mod util;

#[cfg(not(target_arch = "wasm32"))]
mod batcher;

pub use self::chunk::{Chunk, ChunkError, ChunkId, ChunkResult, ChunkTimeline};
pub use self::transport::TransportChunk;
pub use self::util::arrays_to_list_array;

#[cfg(not(target_arch = "wasm32"))]
pub use self::batcher::{
    ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkBatcherResult, PendingRow,
};

pub mod external {
    pub use arrow2;
}
