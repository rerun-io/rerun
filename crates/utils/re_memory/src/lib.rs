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
mod memory_limit;
mod memory_use;
mod ram_warner;
pub mod util;

#[cfg(not(target_arch = "wasm32"))]
mod peak_memory_stats;

#[cfg(not(target_arch = "wasm32"))]
mod backtrace_native;

#[cfg(not(target_arch = "wasm32"))]
use backtrace_native::Backtrace;

#[cfg(target_arch = "wasm32")]
mod backtrace_web;

#[cfg(target_arch = "wasm32")]
use backtrace_web::Backtrace;

pub use self::accounting_allocator::{AccountingAllocator, TrackingStatistics};
pub use self::allocation_tracker::{CallstackStatistics, ReadableBacktrace};
pub use self::memory_limit::MemoryLimit;
pub use self::memory_use::MemoryUse;
#[cfg(not(target_arch = "wasm32"))]
pub use self::peak_memory_stats::PeakMemoryStats;
pub use self::ram_warner::*;

/// Number of allocation and their total size.
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct CountAndSize {
    /// Number of allocations.
    pub count: usize,

    /// Number of bytes.
    pub size: usize,
}

impl std::fmt::Debug for CountAndSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { count, size } = self;
        f.debug_struct("CountAndSize")
            .field("count", &re_format::format_uint(*count))
            .field("size", &re_format::format_bytes(*size as _))
            .finish()
    }
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
    #[inline]
    pub fn new(backtrace: &Backtrace) -> Self {
        Self(ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(backtrace))
    }
}

impl nohash_hasher::IsEnabled for BacktraceHash {}
