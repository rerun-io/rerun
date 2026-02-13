//! Track allocations and memory use.

use std::sync::LazyLock;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use parking_lot::Mutex;

use crate::CountAndSize;
use crate::allocation_tracker::{AllocationTracker, CallstackStatistics, PtrHash};

// TODO(emilk): yet another tier would maybe make sense, with a different stochastic rate.

/// Statistics about extant allocations larger than [`GlobalStats::medium_size`].
static BIG_ALLOCATION_TRACKER: LazyLock<Mutex<AllocationTracker>> =
    LazyLock::new(|| Mutex::new(AllocationTracker::with_stochastic_rate(1)));

/// Statistics about some extant allocations larger than [`GlobalStats::small_size`] but smaller than [`GlobalStats::medium_size`].
static MEDIUM_ALLOCATION_TRACKER: LazyLock<Mutex<AllocationTracker>> =
    LazyLock::new(|| Mutex::new(AllocationTracker::with_stochastic_rate(64)));

thread_local! {
    /// Used to prevent re-entrancy when tracking allocations.
    ///
    /// Tracking an allocation (taking its backtrace etc) can itself create allocations.
    /// We don't want to track those allocations, or we will have infinite recursion.
    static IS_THREAD_IN_ALLOCATION_TRACKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

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

    /// Never capture callstacks for allocations smaller than this.
    pub small_size: AtomicUsize,

    /// Allocations smaller than this are stochastically sampled.
    /// Allocations larger than this are fully sampled.
    pub medium_size: AtomicUsize,

    /// The live allocations not tracked by any [`AllocationTracker`].
    pub untracked: AtomicCountAndSize,

    /// All live allocations sampled by the stochastic medium [`AllocationTracker`].
    pub stochastically_tracked: AtomicCountAndSize,

    /// All live allocations tracked by the large [`AllocationTracker`].
    pub fully_tracked: AtomicCountAndSize,

    /// The live allocations done by [`AllocationTracker`] used for internal book-keeping.
    pub overhead: AtomicCountAndSize,
}

static GLOBAL: GlobalStats = GlobalStats {
    live: AtomicCountAndSize::zero(),
    track_callstacks: AtomicBool::new(false),
    small_size: AtomicUsize::new(1024),
    medium_size: AtomicUsize::new(1024 * 1024),
    untracked: AtomicCountAndSize::zero(),
    stochastically_tracked: AtomicCountAndSize::zero(),
    fully_tracked: AtomicCountAndSize::zero(),
    overhead: AtomicCountAndSize::zero(),
};

// ----------------------------------------------------------------------------

/// Controls which allocations to sample.
///
/// By "sample" we mean "capture a callstack".
#[derive(Clone, Copy, Debug)]
pub struct TrackingOptions {
    /// Never capture callstacks for allocations smaller than this.
    pub small_size: usize,

    /// Allocations smaller than this are stochastically sampled.
    /// Allocations larger than this are fully sampled.
    pub medium_size: usize,
}

impl Default for TrackingOptions {
    fn default() -> Self {
        Self {
            small_size: 1024,
            medium_size: 1024 * 1024,
        }
    }
}

// ----------------------------------------------------------------------------

/// Total number of live allocations,
/// and the number of live bytes allocated as tracked by [`AccountingAllocator`].
///
/// Returns `None` if [`AccountingAllocator`] is not used.
pub fn global_allocs() -> Option<CountAndSize> {
    let count_and_size = GLOBAL.live.load();
    (count_and_size.count > 0).then_some(count_and_size)
}

/// Are we doing (slightly expensive) tracking of the callstacks of large allocations?
pub fn is_tracking_callstacks() -> bool {
    GLOBAL.track_callstacks.load(Relaxed)
}

/// For accurate results, call this BEFORE [`set_tracking_callstacks`].
pub fn set_tracking_options(options: TrackingOptions) {
    let TrackingOptions {
        small_size,
        medium_size,
    } = options;

    GLOBAL.small_size.store(small_size, Relaxed);
    GLOBAL.medium_size.store(medium_size, Relaxed);
}

/// Should we do (slightly expensive) tracking of the callstacks of large allocations?
///
/// To configure it, call [`set_tracking_options`] _first_.
///
/// See also [`turn_on_tracking_if_env_var`].
///
/// Requires that you have installed the [`AccountingAllocator`].
pub fn set_tracking_callstacks(track: bool) {
    let did_track = GLOBAL.track_callstacks.swap(track, Relaxed);
    if !did_track && track {
        re_log::info!("Turning on stochastic tracking of all allocations");
    }
}

/// Turn on callstack tracking (slightly expensive) if a given env-var is set.
///
/// See also [`set_tracking_callstacks`].
///
/// Requires that you have installed the [`AccountingAllocator`].
#[cfg(not(target_arch = "wasm32"))]
pub fn turn_on_tracking_if_env_var(env_var: &str) {
    if std::env::var(env_var).is_ok() {
        set_tracking_callstacks(true);
        re_log::info!("{env_var} found - turning on tracking of all large allocations");
    }
}

// ----------------------------------------------------------------------------

/// Max number of peak callstacks we collect
const MAX_CALLSTACKS: usize = 32;

/// Detailed statistics about memory usage.
///
/// Requires [`set_tracking_callstacks`] having been called.
#[derive(Debug)]
pub struct TrackingStatistics {
    /// Allocations smaller than these are left untracked.
    pub track_size_threshold: usize,

    /// All live allocations that we are NOT tracking (because they were below [`Self::track_size_threshold`]).
    pub untracked: CountAndSize,

    /// All live allocations sampled of medium size, stochastically sampled.
    pub stochastically_tracked: CountAndSize,

    /// All live largish allocations, fully tracked.
    pub fully_tracked: CountAndSize,

    /// All live allocations used for internal book-keeping.
    pub overhead: CountAndSize,

    /// The most popular callstacks.
    pub top_callstacks: Vec<CallstackStatistics>,
}

/// Gather statistics from the live tracking, if enabled.
///
/// Enable this with [`set_tracking_callstacks`], preferably the first thing you do in `main`.
///
/// Requires that you have installed the [`AccountingAllocator`].
pub fn tracking_stats() -> Option<TrackingStatistics> {
    /// NOTE: we use a rather large [`smallvec::SmallVec`] here to avoid dynamic allocations,
    /// which would otherwise confuse the memory tracking.
    fn tracker_stats(
        allocation_tracker: &AllocationTracker,
    ) -> smallvec::SmallVec<[CallstackStatistics; MAX_CALLSTACKS]> {
        let top_callstacks: smallvec::SmallVec<[CallstackStatistics; MAX_CALLSTACKS]> =
            allocation_tracker
                .top_callstacks(MAX_CALLSTACKS)
                .into_iter()
                .collect();
        assert!(
            !top_callstacks.spilled(),
            "We shouldn't leak any allocations"
        );
        top_callstacks
    }

    GLOBAL.track_callstacks.load(Relaxed).then(|| {
        IS_THREAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
            // prevent double-lock of ALLOCATION_TRACKER:
            is_thread_in_allocation_tracker.set(true);
            let mut top_big_callstacks = tracker_stats(&BIG_ALLOCATION_TRACKER.lock());
            let mut top_medium_callstacks = tracker_stats(&MEDIUM_ALLOCATION_TRACKER.lock());
            is_thread_in_allocation_tracker.set(false);

            let mut top_callstacks: Vec<_> = top_big_callstacks
                .drain(..)
                .chain(top_medium_callstacks.drain(..))
                .collect();

            #[expect(clippy::cast_possible_wrap)]
            top_callstacks.sort_by_key(|c| -(c.estimated().size as i64));

            TrackingStatistics {
                track_size_threshold: GLOBAL.small_size.load(Relaxed),
                untracked: GLOBAL.untracked.load(),
                stochastically_tracked: GLOBAL.stochastically_tracked.load(),
                fully_tracked: GLOBAL.fully_tracked.load(),
                overhead: GLOBAL.overhead.load(),
                top_callstacks,
            }
        })
    })
}

// ----------------------------------------------------------------------------

/// Install this as the global allocator to get memory usage tracking.
///
/// Use [`set_tracking_callstacks`] or [`turn_on_tracking_if_env_var`] to turn on memory tracking.
/// Collect the stats with [`tracking_stats`].
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

#[expect(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl<InnerAllocator: std::alloc::GlobalAlloc> std::alloc::GlobalAlloc
    for AccountingAllocator<InnerAllocator>
{
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
    GLOBAL.live.add(size);

    if GLOBAL.track_callstacks.load(Relaxed) {
        if size < GLOBAL.small_size.load(Relaxed) {
            // Too small to track.
            GLOBAL.untracked.add(size);
        } else {
            // Big enough to track - but make sure we don't create a deadlock by trying to
            // track the allocations made by the allocation tracker:

            IS_THREAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
                if !is_thread_in_allocation_tracker.get() {
                    is_thread_in_allocation_tracker.set(true);

                    let ptr_hash = PtrHash::new(ptr);
                    if size < GLOBAL.medium_size.load(Relaxed) {
                        GLOBAL.stochastically_tracked.add(size);
                        MEDIUM_ALLOCATION_TRACKER.lock().on_alloc(ptr_hash, size);
                    } else {
                        GLOBAL.fully_tracked.add(size);
                        BIG_ALLOCATION_TRACKER.lock().on_alloc(ptr_hash, size);
                    }

                    is_thread_in_allocation_tracker.set(false);
                } else {
                    // This is the ALLOCATION_TRACKER allocating memory.
                    GLOBAL.overhead.add(size);
                }
            });
        }
    }
}

#[inline]
fn note_dealloc(ptr: *mut u8, size: usize) {
    GLOBAL.live.sub(size);

    if GLOBAL.track_callstacks.load(Relaxed) {
        if size < GLOBAL.small_size.load(Relaxed) {
            // Too small to track.
            GLOBAL.untracked.sub(size);
        } else {
            // Big enough to track - but make sure we don't create a deadlock by trying to
            // track the allocations made by the allocation tracker:
            IS_THREAD_IN_ALLOCATION_TRACKER.with(|is_thread_in_allocation_tracker| {
                if !is_thread_in_allocation_tracker.get() {
                    is_thread_in_allocation_tracker.set(true);

                    let ptr_hash = PtrHash::new(ptr);
                    if size < GLOBAL.medium_size.load(Relaxed) {
                        GLOBAL.stochastically_tracked.sub(size);
                        MEDIUM_ALLOCATION_TRACKER.lock().on_dealloc(ptr_hash, size);
                    } else {
                        GLOBAL.fully_tracked.sub(size);
                        BIG_ALLOCATION_TRACKER.lock().on_dealloc(ptr_hash, size);
                    }

                    is_thread_in_allocation_tracker.set(false);
                } else {
                    // This is the ALLOCATION_TRACKER freeing memory.
                    GLOBAL.overhead.sub(size);
                }
            });
        }
    }
}
