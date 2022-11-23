//! Track allocations and memory use.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use crate::{
    allocation_tracker::{AllocationTracker, CallstackStatistics},
    CountAndSize,
};

/// Only track allocations of at least this size.
const TRACK_MINIMUM: usize = 128; // TODO(emilk): make this setable by users

// ----------------------------------------------------------------------------

struct AtomicCountAndSize {
    /// Number of allocations.
    pub count: AtomicUsize,

    /// Number of bytes.
    pub size: AtomicUsize,
}

impl AtomicCountAndSize {
    pub const fn zero() -> Self {
        Self {
            count: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
        }
    }

    fn load(&self) -> CountAndSize {
        CountAndSize {
            count: self.count.load(Relaxed),
            size: self.size.load(Relaxed),
        }
    }

    /// Add an allocation.
    fn add(&self, size: usize) {
        self.count.fetch_add(1, Relaxed);
        self.size.fetch_add(size, Relaxed);
    }

    /// Remove an allocation.
    fn sub(&self, size: usize) {
        self.count.fetch_sub(1, Relaxed);
        self.size.fetch_sub(size, Relaxed);
    }
}

struct GlobalStats {
    /// All extant allocations.
    pub live: AtomicCountAndSize,

    /// Do detailed statistics of allocations?
    /// This is expensive, but sometimes useful!
    pub track_callstacks: AtomicBool,

    /// The live allocations not tracked by [`AllocationTracker`].
    pub untracked: AtomicCountAndSize,

    /// The live allocations done by [`AllocationTracker`] used for internal book-keeping.
    pub overhead: AtomicCountAndSize,
}

// ----------------------------------------------------------------------------

static GLOBAL_STATS: GlobalStats = GlobalStats {
    live: AtomicCountAndSize::zero(),
    track_callstacks: AtomicBool::new(false),
    untracked: AtomicCountAndSize::zero(),
    overhead: AtomicCountAndSize::zero(),
};

/// Total number of live allocations,
/// and the number of live bytes allocated as tracked by [`AccountingAllocator`].
///
/// Returns (0,0) if [`AccountingAllocator`] is not used.
pub fn global_allocs() -> CountAndSize {
    GLOBAL_STATS.live.load()
}

/// Are we doing (rather expensive) tracking of the callstacks of large allocations?
pub fn is_tracking_callstacks() -> bool {
    GLOBAL_STATS.track_callstacks.load(Relaxed)
}

/// Should we do (rather expensive) tracking of the callstacks of large allocations?
pub fn set_tracking_callstacks(track: bool) {
    GLOBAL_STATS.track_callstacks.store(track, Relaxed);
}

/// Turn on callstack tracking (rather expensive) if a given env-var is set.
#[cfg(not(target_arch = "wasm32"))]
pub fn turn_on_tracking_if_env_var(env_var: &str) {
    if std::env::var(env_var).is_ok() {
        set_tracking_callstacks(true);
        re_log::info!("{env_var} found - turning on tracking of all large allocations");
    }
}

// ----------------------------------------------------------------------------

/// Statistics about extant allocations.
///
/// Activated by [`GlobalStats::track_callstacks`].
static ALLOCATION_TRACKER: once_cell::sync::Lazy<parking_lot::Mutex<AllocationTracker>> =
    once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(AllocationTracker::default()));

thread_local! {
    /// Used to prevent re-entrancy when tracking allocations.
    ///
    /// Tracking an allocation (taking its backtrace etc) can itself create allocations.
    /// We don't want to track those allocations, or we will have infinite recursion.
    static IS_TRHEAD_IN_ALLOCATION_TRACKER: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

const MAX_CALLSTACKS: usize = 128;

pub struct TrackingStatistics {
    /// All live allocations that we are tracking.
    pub tracked: CountAndSize,

    /// All live allocations that we are NOT tracking (because they are too small).
    pub untracked: CountAndSize,

    /// All live allocations used for internal book-keeping.
    pub overhead: CountAndSize,

    /// Allocations smaller than these are left untracked.
    pub track_size_threshold: usize,

    /// The most popular callstacks.
    ///
    /// NOTE: we use a rather large [`smallvec::SmallVec`] here to avoid dynamic allocations,
    /// which would otherwise confuse the memory tracking.
    pub top_callstacks: smallvec::SmallVec<[CallstackStatistics; MAX_CALLSTACKS]>,
}

/// Gather statistics from the live tracking, if enabled.
///
/// Enable this with [`set_tracking_callstacks`], preferably the first thing you do in `main`.
pub fn tracking_stats(max_callstacks: usize) -> Option<TrackingStatistics> {
    GLOBAL_STATS.track_callstacks.load(Relaxed).then(|| {
        IS_TRHEAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
            // prevent double-lock of ALLOCATION_TRACKER:
            is_thread_in_allocation_tracker.set(true);

            let untracked = GLOBAL_STATS.untracked.load();
            let overhead = GLOBAL_STATS.overhead.load();

            let allocation_tracker = ALLOCATION_TRACKER.lock();
            let tracked = allocation_tracker.tracked_allocs_and_bytes();
            let top_callstacks: smallvec::SmallVec<[CallstackStatistics; MAX_CALLSTACKS]> =
                allocation_tracker
                    .top_callstacks(max_callstacks.min(MAX_CALLSTACKS))
                    .into_iter()
                    .collect();

            assert!(
                !top_callstacks.spilled(),
                "We shouldn't leak any allocations"
            );

            is_thread_in_allocation_tracker.set(false);

            TrackingStatistics {
                tracked,
                untracked,
                overhead,
                track_size_threshold: TRACK_MINIMUM,
                top_callstacks,
            }
        })
    })
}

// ----------------------------------------------------------------------------

/// Install this as the global allocator to get memory usage tracking.
///
/// Usage:
/// ```
/// use re_memory::AccountingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: AccountingAllocator<std::alloc::System> = AccountingAllocator::new(std::alloc::System);
/// ```
#[derive(Default)]
pub struct AccountingAllocator<InnerAllocator> {
    allocator: InnerAllocator,
}

impl<InnerAllocator> AccountingAllocator<InnerAllocator> {
    pub const fn new(allocator: InnerAllocator) -> Self {
        Self { allocator }
    }
}

#[allow(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl<InnerAllocator: std::alloc::GlobalAlloc> std::alloc::GlobalAlloc
    for AccountingAllocator<InnerAllocator>
{
    #[allow(clippy::let_and_return)]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        let ptr = unsafe { self.allocator.alloc(layout) };

        note_alloc(ptr, layout.size());

        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        let ptr = unsafe { self.allocator.alloc_zeroed(layout) };

        note_alloc(ptr, layout.size());

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        unsafe { self.allocator.dealloc(ptr, layout) };

        note_dealloc(ptr, layout.size());
    }

    unsafe fn realloc(
        &self,
        old_ptr: *mut u8,
        layout: std::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        note_dealloc(old_ptr, layout.size());

        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        let new_ptr = unsafe { self.allocator.realloc(old_ptr, layout, new_size) };

        note_alloc(new_ptr, new_size);

        new_ptr
    }
}

#[inline]
fn note_alloc(ptr: *mut u8, size: usize) {
    GLOBAL_STATS.live.add(size);

    if GLOBAL_STATS.track_callstacks.load(Relaxed) {
        if size < TRACK_MINIMUM {
            // Too small to track.
            GLOBAL_STATS.untracked.add(size);
        } else {
            // TODO(emilk): stochastically sample medium-sized allocations (e.g. < 16 kB) based on pointer hash.

            // Big enough to track - but make sure we don't create a deadlock by trying to
            // track the allocations made by the allocation tracker:
            IS_TRHEAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
                if !is_thread_in_allocation_tracker.get() {
                    is_thread_in_allocation_tracker.set(true);
                    ALLOCATION_TRACKER.lock().on_alloc(ptr as usize, size);
                    is_thread_in_allocation_tracker.set(false);
                } else {
                    // This is the ALLOCATION_TRACKER allocating memory.
                    GLOBAL_STATS.overhead.add(size);
                }
            });
        }
    }
}

#[inline]
fn note_dealloc(ptr: *mut u8, size: usize) {
    GLOBAL_STATS.live.sub(size);

    if GLOBAL_STATS.track_callstacks.load(Relaxed) {
        if size < TRACK_MINIMUM {
            // Too small to track.
            GLOBAL_STATS.untracked.sub(size);
        } else {
            // Big enough to track - but make sure we don't create a deadlock by trying to
            // track the allocations made by the allocation tracker:
            IS_TRHEAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
                if !is_thread_in_allocation_tracker.get() {
                    is_thread_in_allocation_tracker.set(true);
                    ALLOCATION_TRACKER.lock().on_dealloc(ptr as usize, size);
                    is_thread_in_allocation_tracker.set(false);
                } else {
                    // This is the ALLOCATION_TRACKER freeing memory.
                    GLOBAL_STATS.overhead.sub(size);
                }
            });
        }
    }
}
