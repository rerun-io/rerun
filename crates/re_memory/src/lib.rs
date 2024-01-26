//! Run-time memory tracking and profiling.
//!
//! ## First steps
//!
//! Add `re_memory` to your `Cargo.toml`:
//!
//! ```toml
//! cargo add re_memory
//! ```
//!
//! Install the [`AccountingAllocator`] in your `main.rs`:
//! ```no_run
//! use re_memory::AccountingAllocator;
//!
//! #[global_allocator]
//! static GLOBAL: AccountingAllocator<std::alloc::System>
//!     = AccountingAllocator::new(std::alloc::System);
//! ```
//!
//! ### Checking memory use
//! Use [`MemoryUse::capture`] to get the current memory use of your application.
//!
//! ### Finding memory leaks
//! Turn on memory tracking at the top of your `main()` function:
//!
//! ```rs
//! re_memory::accounting_allocator::set_tracking_callstacks(true);
//! ```
//!
//! Now let your app run for a while, and then call [`accounting_allocator::tracking_stats`]
//! to get the statistics. Any memory leak should show up in
//! [`TrackingStatistics::top_callstacks`].
//!
//! ### More
//! See also [`accounting_allocator`].

pub mod accounting_allocator;
mod allocation_tracker;
mod memory_history;
mod memory_limit;
mod memory_use;
mod ram_warner;
pub mod util;

#[cfg(not(target_arch = "wasm32"))]
mod backtrace_native;

#[cfg(not(target_arch = "wasm32"))]
use backtrace_native::Backtrace;

#[cfg(target_arch = "wasm32")]
mod backtrace_web;

#[cfg(target_arch = "wasm32")]
use backtrace_web::Backtrace;

pub use {
    accounting_allocator::{AccountingAllocator, TrackingStatistics},
    memory_history::MemoryHistory,
    memory_limit::MemoryLimit,
    memory_use::MemoryUse,
    ram_warner::*,
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

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct BacktraceHash(u64);

impl BacktraceHash {
    pub fn new(backtrace: &Backtrace) -> Self {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher =
            std::hash::BuildHasher::build_hasher(&ahash::RandomState::with_seeds(0, 1, 2, 3));
        backtrace.hash(&mut hasher);
        Self(hasher.finish())
    }
}

impl nohash_hasher::IsEnabled for BacktraceHash {}
