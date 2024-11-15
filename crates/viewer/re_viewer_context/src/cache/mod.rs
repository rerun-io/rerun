//! Viewer caches
//!
//! Caches are registered lazily upon first use, see [`Caches::entry`].
//! The concrete caches exposed here are always available for all viewer crates.

mod caches;
mod image_decode_cache;
mod image_stats_cache;
mod tensor_stats_cache;
mod video_cache;

pub use caches::{Cache, Caches};

// TODO(andreas): Do we _really_ have to have all these caches in `re_viewer_context`?
// Caches are fully dynamic and registration based, so they can be added at runtime by any crate.
// The reason this happens it that various viewer crates wants to access these, mostly for ui purposes.
// Ideally, they would only depend on the ones needed.
pub use image_decode_cache::ImageDecodeCache;
pub use image_stats_cache::ImageStatsCache;
pub use tensor_stats_cache::TensorStatsCache;
pub use video_cache::VideoCache;
