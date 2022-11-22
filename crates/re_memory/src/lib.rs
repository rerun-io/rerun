//! Crate for tracking memory use.

mod allocation_tracker;
mod memory_history;
mod memory_limit;
mod memory_use;
pub mod tracking_allocator;
pub mod util;

pub use {
    memory_history::MemoryHistory, memory_limit::MemoryLimit, memory_use::MemoryUse,
    tracking_allocator::TrackingAllocator,
};

/// Number of allocation and their total size.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CountAndSize {
    /// Number of allocations.
    pub count: usize,

    /// Number of bytes.
    pub size: usize,
}

impl CountAndSize {
    pub const ZERO: Self = Self { count: 0, size: 0 };
}
