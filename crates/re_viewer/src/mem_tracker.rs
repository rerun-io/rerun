//! Track allocations and memory use.

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static GLOBAL_STATS: GlobalStats = GlobalStats::new();

// ----------------------------------------------------------------------------

struct GlobalStats {
    /// Total number of allocations minus number of frees.
    pub live_allocs: AtomicUsize,

    /// Total bytes allocated minus those freed.
    pub live_bytes: AtomicUsize,
}

impl GlobalStats {
    pub const fn new() -> Self {
        Self {
            live_allocs: AtomicUsize::new(0),
            live_bytes: AtomicUsize::new(0),
        }
    }
}

/// Total number of live allocations,
/// and the number of live bytes allocated.
pub fn global_allocs_and_bytes() -> (usize, usize) {
    (
        GLOBAL_STATS.live_allocs.load(Relaxed),
        GLOBAL_STATS.live_bytes.load(Relaxed),
    )
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
        GLOBAL_STATS.live_allocs.fetch_add(1, Relaxed);
        GLOBAL_STATS.live_bytes.fetch_add(layout.size(), Relaxed);

        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        GLOBAL_STATS.live_allocs.fetch_sub(1, Relaxed);
        GLOBAL_STATS.live_bytes.fetch_sub(layout.size(), Relaxed);

        // SAFETY:
        // We just do book-keeping and then let another allocator do all the actual work.
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}
