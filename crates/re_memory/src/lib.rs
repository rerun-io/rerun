//! Run-time memory tracking and profiling.
//!
//! See [`AccountingAllocator`] and [`accounting_allocator`].

pub mod accounting_allocator;
mod allocation_tracker;
mod memory_history;
mod memory_limit;
mod memory_use;
pub mod util;

pub use {
    accounting_allocator::AccountingAllocator, memory_history::MemoryHistory,
    memory_limit::MemoryLimit, memory_use::MemoryUse,
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

    /// Add an allocation.
    #[inline]
    pub fn add(&mut self, size: usize) {
        self.count += 1;
        self.size += size;
    }

    /// Remove an allocation.
    #[inline]
    pub fn sub(&mut self, size: usize) {
        self.count -= 1;
        self.size -= size;
    }
}
