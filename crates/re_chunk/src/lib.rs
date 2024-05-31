//! A chunk of Rerun data, encoded using Arrow. Used for logging, transport, storage and compute.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod chunk;
mod shuffle;
mod util;

pub use self::chunk::{Chunk, ChunkError, ChunkId, ChunkResult, ChunkTimeline};
pub use self::util::arrays_to_list_array;

pub mod external {
    pub use arrow2;
}
