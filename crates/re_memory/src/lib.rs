//! Crate for tracking memory use.
mod tracking_allocator;

pub use tracking_allocator::{global_allocs_and_bytes, TrackingAllocator};
