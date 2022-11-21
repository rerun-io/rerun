//! Crate for tracking memory use.
mod memory_use;
mod tracking_allocator;

pub use {
    memory_use::MemoryUse,
    tracking_allocator::{global_allocs_and_bytes, TrackingAllocator},
};
