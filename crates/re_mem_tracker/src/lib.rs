//! Track allocations and memory use.

mod stats_tree;
pub use stats_tree::*;

use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicIsize, AtomicUsize, Ordering::SeqCst},
        Arc,
    },
};

thread_local! {
     static THREAD_ALLOC_STATS: InnerAllocStats = InnerAllocStats::new();
}

static GLOBAL_STATS: GlobalStats = GlobalStats::new();

// ----------------------------------------------------------------------------

struct GlobalStats {
    /// Total number of allocations minus number of frees.
    pub total_allocs: AtomicUsize,

    /// Total bytes allocated minus those freed.
    pub total_bytes: AtomicUsize,
}

impl GlobalStats {
    pub const fn new() -> Self {
        Self {
            total_allocs: AtomicUsize::new(0),
            total_bytes: AtomicUsize::new(0),
        }
    }
}

/// Total number of live allocations,
/// and the number of bytes allocated.
pub fn global_allocs_and_bytes() -> (usize, usize) {
    (
        GLOBAL_STATS.total_allocs.load(SeqCst),
        GLOBAL_STATS.total_bytes.load(SeqCst),
    )
}

// ----------------------------------------------------------------------------

#[derive(Default)]
struct InnerAllocStats {
    /// Total number of allocations minus number of frees.
    pub total_allocs: AtomicIsize,

    /// Total bytes allocated minus those freed.
    pub total_bytes: AtomicIsize,

    /// The allocations that no child scope has accounted for.
    pub unaccounted_allocs: AtomicIsize,

    /// The bytes that no child scope has accounted for.
    pub unaccounted_bytes: AtomicIsize,
}

impl Clone for InnerAllocStats {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            total_allocs: AtomicIsize::new(self.total_allocs.load(SeqCst)),
            total_bytes: AtomicIsize::new(self.total_bytes.load(SeqCst)),
            unaccounted_allocs: AtomicIsize::new(self.unaccounted_allocs.load(SeqCst)),
            unaccounted_bytes: AtomicIsize::new(self.unaccounted_bytes.load(SeqCst)),
        }
    }
}

impl InnerAllocStats {
    pub const fn new() -> Self {
        Self {
            total_allocs: AtomicIsize::new(0),
            total_bytes: AtomicIsize::new(0),
            unaccounted_allocs: AtomicIsize::new(0),
            unaccounted_bytes: AtomicIsize::new(0),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct AllocStats(Arc<InnerAllocStats>);

impl AllocStats {
    /// Total number of allocations minus number of frees.
    pub fn total_allocs(&self) -> isize {
        self.0.total_allocs.load(SeqCst)
    }

    /// Total bytes allocated minus those freed.
    pub fn total_bytes(&self) -> isize {
        self.0.total_bytes.load(SeqCst)
    }

    /// The allocations that no child scope has accounted for.
    pub fn unaccounted_allocs(&self) -> isize {
        self.0.unaccounted_allocs.load(SeqCst)
    }

    /// The bytes that no child scope has accounted for.
    pub fn unaccounted_bytes(&self) -> isize {
        self.0.unaccounted_bytes.load(SeqCst)
    }
}

// ----------------------------------------------------------------------------

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
        GLOBAL_STATS.total_allocs.fetch_add(1, SeqCst);
        GLOBAL_STATS.total_bytes.fetch_add(layout.size(), SeqCst);

        THREAD_ALLOC_STATS.with(|thread_stats| {
            thread_stats.total_allocs.fetch_add(1, SeqCst);
            thread_stats
                .total_bytes
                .fetch_add(layout.size() as _, SeqCst);
            thread_stats.unaccounted_allocs.fetch_add(1, SeqCst);
            thread_stats
                .unaccounted_bytes
                .fetch_add(layout.size() as _, SeqCst);
        });

        self.allocator.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        GLOBAL_STATS.total_allocs.fetch_sub(1, SeqCst);
        GLOBAL_STATS.total_bytes.fetch_sub(layout.size(), SeqCst);

        THREAD_ALLOC_STATS.with(|thread_stats| {
            thread_stats.total_allocs.fetch_sub(1, SeqCst);
            thread_stats
                .total_bytes
                .fetch_sub(layout.size() as _, SeqCst);
            thread_stats.unaccounted_allocs.fetch_sub(1, SeqCst);
            thread_stats
                .unaccounted_bytes
                .fetch_sub(layout.size() as _, SeqCst);
        });

        self.allocator.dealloc(ptr, layout);
    }
}

// ----------------------------------------------------------------------------

thread_local! {
    #[doc(hidden)]
    pub static THREAD_TREE: RefCell<Tree> = Default::default();
}

pub fn thread_local_tree() -> Tree {
    THREAD_TREE.with(|thread_stats| thread_stats.borrow().clone())
}

#[macro_export]
macro_rules! track_allocs {
    ($name:expr) => {
        let _tracker_scope = $crate::TrackerScope::new(
            &$crate::THREAD_TREE
                .with(|thread_stats| thread_stats.borrow_mut().child($name).stats.clone()),
        );
    };
}

/// Created by the [`re_mem_tracker::track_allocs`].
pub struct TrackerScope {
    target: Arc<InnerAllocStats>,

    start_stats: InnerAllocStats,

    /// Prevent the scope from being sent between threads.
    /// The scope must start/stop on the same thread.
    /// In particular, we do NOT want this to migrate threads in some async code.
    /// Workaround until `impl !Send for TrackerScope {}` is stable.
    _dont_send_me: std::marker::PhantomData<*const ()>,
}

impl TrackerScope {
    /// The `id` doesn't need to be static, but it should be unchanging,
    /// and this is a good way to enforce it.
    /// `data` can be changing, i.e. a name of a mesh or a texture.
    #[inline]
    pub fn new(target: &AllocStats) -> Self {
        Self {
            target: target.0.clone(),
            start_stats: THREAD_ALLOC_STATS.with(|thread_stats| thread_stats.clone()),
            _dont_send_me: Default::default(),
        }
    }
}

impl Drop for TrackerScope {
    #[inline]
    fn drop(&mut self) {
        let stop_state = THREAD_ALLOC_STATS.with(|thread_stats| {
            let stop_state = thread_stats.clone();
            thread_stats
                .unaccounted_allocs
                .store(self.start_stats.unaccounted_allocs.load(SeqCst), SeqCst); // we will account for them
            thread_stats
                .unaccounted_bytes
                .store(self.start_stats.unaccounted_bytes.load(SeqCst), SeqCst); // we will account for them
            stop_state
        });
        let target: &InnerAllocStats = &*self.target;

        target.total_allocs.fetch_add(
            stop_state.total_allocs.load(SeqCst) - self.start_stats.total_allocs.load(SeqCst),
            SeqCst,
        );
        target.total_bytes.fetch_add(
            stop_state.total_bytes.load(SeqCst) - self.start_stats.total_bytes.load(SeqCst),
            SeqCst,
        );
        target.unaccounted_allocs.fetch_add(
            stop_state.unaccounted_allocs.load(SeqCst)
                - self.start_stats.unaccounted_allocs.load(SeqCst),
            SeqCst,
        );
        target.unaccounted_bytes.fetch_add(
            stop_state.unaccounted_bytes.load(SeqCst)
                - self.start_stats.unaccounted_bytes.load(SeqCst),
            SeqCst,
        );
    }
}

// ----------------------------------------------------------------------------

/// Ignore all allocations within this scope.
///
/// Useful if you plan to deallocate them somewhere else, and so they shouldn't count.
#[macro_export]
macro_rules! ignore {
    () => {
        let _ignore_scope = $crate::IgnoreScope::default();
    };
}

/// Created by the [`re_mem_tracker::ignore`].
pub struct IgnoreScope {
    start_stats: InnerAllocStats,

    /// Prevent the scope from being sent between threads.
    /// The scope must start/stop on the same thread.
    /// In particular, we do NOT want this to migrate threads in some async code.
    /// Workaround until `impl !Send for IgnoreScope {}` is stable.
    _dont_send_me: std::marker::PhantomData<*const ()>,
}

impl Default for IgnoreScope {
    /// The `id` doesn't need to be static, but it should be unchanging,
    /// and this is a good way to enforce it.
    /// `data` can be changing, i.e. a name of a mesh or a texture.
    #[inline]
    fn default() -> Self {
        Self {
            start_stats: THREAD_ALLOC_STATS.with(|thread_stats| thread_stats.clone()),
            _dont_send_me: Default::default(),
        }
    }
}

impl Drop for IgnoreScope {
    #[inline]
    fn drop(&mut self) {
        THREAD_ALLOC_STATS.with(|thread_stats| {
            thread_stats
                .total_allocs
                .store(self.start_stats.total_allocs.load(SeqCst), SeqCst);
            thread_stats
                .total_bytes
                .store(self.start_stats.total_bytes.load(SeqCst), SeqCst);
            thread_stats
                .unaccounted_allocs
                .store(self.start_stats.unaccounted_allocs.load(SeqCst), SeqCst);
            thread_stats
                .unaccounted_bytes
                .store(self.start_stats.unaccounted_bytes.load(SeqCst), SeqCst);
        });
    }
}
