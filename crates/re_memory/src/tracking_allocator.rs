//! Track allocations and memory use.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use crate::{allocation_tracker::CallstackStatistics, AllocationTracker};

/// Only track allocations of at least this size.
const TRACK_MINIMUM: usize = 128;

// ----------------------------------------------------------------------------

struct GlobalStats {
    /// Total number of allocations minus number of frees.
    pub live_allocs: AtomicUsize,

    /// Total bytes allocated minus those freed.
    pub live_bytes: AtomicUsize,

    /// Do detailed statistics of allocations?
    /// This is expensive, but sometimes useful!
    pub track_callstacks: AtomicBool,

    /// Number of live allocations not tracked by [`AllocationTracker`].
    pub untracked_allocs: AtomicUsize,

    /// Number of live bytes not tracked by [`AllocationTracker`].
    pub untracked_bytes: AtomicUsize,
}

// ----------------------------------------------------------------------------

static GLOBAL_STATS: GlobalStats = GlobalStats {
    live_allocs: AtomicUsize::new(0),
    live_bytes: AtomicUsize::new(0),
    track_callstacks: AtomicBool::new(true), // TODO: check an env-var during startup!
    untracked_allocs: AtomicUsize::new(0),
    untracked_bytes: AtomicUsize::new(0),
};

/// Total number of live allocations,
/// and the number of live bytes allocated as tracked by [`TrackingAllocator`].
///
/// Returns (0,0) if [`TrackingAllocator`] is not used.
pub fn global_allocs_and_bytes() -> (usize, usize) {
    (
        GLOBAL_STATS.live_allocs.load(Relaxed),
        GLOBAL_STATS.live_bytes.load(Relaxed),
    )
}

pub fn is_tracking_callstacks() -> bool {
    GLOBAL_STATS.track_callstacks.load(Relaxed)
}

// ----------------------------------------------------------------------------

thread_local! {
    /// Used to prevent re-entrancy when tracking allocations.
    ///
    /// Tracking an allocation (taking its backtrace etc) can itself create allocations.
    /// We don't want to track those allocations, or we will have infinite recursion.
    static NUM_THREAD_REENTRANCY: std::cell::RefCell<usize> = std::cell::RefCell::new(0);
}

/// Statistics about extant allocations.
///
/// Activated by [`GlobalStats::track_callstacks`].
static ALLOCATION_TRACKER: once_cell::sync::Lazy<parking_lot::Mutex<AllocationTracker>> =
    once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(AllocationTracker::default()));

pub struct TrackingStatistics {
    /// How many live allocations are we tracking?
    pub tracked_allocs: usize,

    /// How many live bytes are we tracking?
    pub tracked_bytes: usize,

    /// How many live allocations are we NOT tracking (because they are too small)?
    pub untracked_allocs: usize,

    /// How many live bytes are we NOT tracking (because they are too small)?
    pub untracked_bytes: usize,

    /// Allocations smaller than these are left untracked.
    pub track_size_threshold: usize,

    /// The most popular callstacks.
    pub top_callstacks: Vec<CallstackStatistics>,
}

/// Gather statistics from the live tracking, if enabled.
pub fn tracking_stats(max_callstacks: usize) -> Option<TrackingStatistics> {
    GLOBAL_STATS.track_callstacks.load(Relaxed).then(|| {
        NUM_THREAD_REENTRANCY.with(|num_thread_reentrancy| {
            // prevent double-lock of ALLOCATION_TRACKER:
            *num_thread_reentrancy.borrow_mut() += 1;

            let untracked_allocs = GLOBAL_STATS.untracked_allocs.load(Relaxed);
            let untracked_bytes = GLOBAL_STATS.untracked_bytes.load(Relaxed);

            let allocation_tracker = ALLOCATION_TRACKER.lock();
            let (tracked_allocs, tracked_bytes) = allocation_tracker.tracked_allocs_and_bytes();
            let top_callstacks = allocation_tracker
                .top_callstacks(max_callstacks)
                .iter()
                .cloned()
                .cloned()
                .collect();

            *num_thread_reentrancy.borrow_mut() -= 1;

            TrackingStatistics {
                tracked_allocs,
                tracked_bytes,
                untracked_allocs,
                untracked_bytes,
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
/// use re_memory::TrackingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: TrackingAllocator<std::alloc::System> = TrackingAllocator::new(std::alloc::System);
/// ```
#[derive(Default)]
pub struct TrackingAllocator<InnerAllocator> {
    allocator: InnerAllocator,
}

impl<InnerAllocator> TrackingAllocator<InnerAllocator> {
    pub const fn new(allocator: InnerAllocator) -> Self {
        Self { allocator }
    }
}

#[allow(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl<InnerAllocator: std::alloc::GlobalAlloc> std::alloc::GlobalAlloc
    for TrackingAllocator<InnerAllocator>
{
    #[allow(clippy::let_and_return)]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        let ptr = unsafe { self.allocator.alloc(layout) };

        GLOBAL_STATS.live_allocs.fetch_add(1, Relaxed);
        GLOBAL_STATS.live_bytes.fetch_add(layout.size(), Relaxed);

        if GLOBAL_STATS.track_callstacks.load(Relaxed) {
            if layout.size() < TRACK_MINIMUM {
                GLOBAL_STATS.untracked_allocs.fetch_add(1, Relaxed);
                GLOBAL_STATS
                    .untracked_bytes
                    .fetch_add(layout.size(), Relaxed);
            } else {
                // TODO: track how much memory falls below TRACK_MINIMUM
                // TODO: keep track of how much memory is used by the allocation tracker
                NUM_THREAD_REENTRANCY.with(|num_thread_reentrancy| {
                    if *num_thread_reentrancy.borrow() > 0 {
                        return; // prevent dead-lock
                    } else {
                        *num_thread_reentrancy.borrow_mut() += 1;
                    }

                    ALLOCATION_TRACKER.lock().on_alloc(ptr as usize, &layout);

                    *num_thread_reentrancy.borrow_mut() -= 1;
                });
            }
        }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        unsafe { self.allocator.dealloc(ptr, layout) };

        GLOBAL_STATS.live_allocs.fetch_sub(1, Relaxed);
        GLOBAL_STATS.live_bytes.fetch_sub(layout.size(), Relaxed);

        if GLOBAL_STATS.track_callstacks.load(Relaxed) {
            if layout.size() < TRACK_MINIMUM {
                GLOBAL_STATS.untracked_allocs.fetch_sub(1, Relaxed);
                GLOBAL_STATS
                    .untracked_bytes
                    .fetch_sub(layout.size(), Relaxed);
            } else {
                NUM_THREAD_REENTRANCY.with(|num_thread_reentrancy| {
                    if *num_thread_reentrancy.borrow() > 0 {
                        return; // prevent dead-lock
                    } else {
                        *num_thread_reentrancy.borrow_mut() += 1;
                    }

                    ALLOCATION_TRACKER.lock().on_dealloc(ptr as usize, &layout);

                    *num_thread_reentrancy.borrow_mut() -= 1;
                });
            }
        }
    }
}
