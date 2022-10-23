use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

#[derive(Default)]
pub struct AllocatorStats {
    /// Total number of allocations over all time. Increases monotonically.
    cumul_alloc_count: AtomicUsize,

    /// Total number of calls to free over all time. Increases monotonically.
    cumul_dealloc_count: AtomicUsize,

    /// Total number of bytes that has been allocated over all time. Increases monotonically.
    cumul_alloc_size: AtomicUsize,

    /// Total number of bytes that has been freed over all time. Increases monotonically.
    cumul_dealloc_size: AtomicUsize,

    /// Most bytes used at any one time. Increases monotonically.
    high_water_mark_bytes: AtomicUsize,
}

impl AllocatorStats {
    pub const fn new() -> Self {
        Self {
            cumul_alloc_count: AtomicUsize::new(0),
            cumul_dealloc_count: AtomicUsize::new(0),
            cumul_alloc_size: AtomicUsize::new(0),
            cumul_dealloc_size: AtomicUsize::new(0),
            high_water_mark_bytes: AtomicUsize::new(0),
        }
    }

    /// Number of bytes allocated currently.
    #[inline]
    pub fn num_bytes_now(&self) -> usize {
        self.cumul_alloc_size.load(SeqCst) - self.cumul_dealloc_size.load(SeqCst)
    }

    #[inline]
    pub fn current_allocations(&self) -> usize {
        self.cumul_alloc_count.load(SeqCst) - self.cumul_dealloc_count.load(SeqCst)
    }
}

#[derive(Default)]
pub struct TrackingAllocator<InnerAllocator> {
    allocator: InnerAllocator,
    stats: AllocatorStats,
}

impl<InnerAllocator> TrackingAllocator<InnerAllocator> {
    pub const fn new(allocator: InnerAllocator) -> Self {
        Self {
            allocator,
            stats: AllocatorStats::new(),
        }
    }

    /// Number of bytes allocated currently.
    #[inline]
    pub fn num_bytes_now(&self) -> usize {
        self.stats.num_bytes_now()
    }

    #[inline]
    pub fn current_allocations(&self) -> usize {
        self.stats.current_allocations()
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
        self.stats.cumul_alloc_count.fetch_add(1, SeqCst);
        self.stats.cumul_alloc_size.fetch_add(layout.size(), SeqCst);

        let used = self.stats.num_bytes_now();
        self.stats.high_water_mark_bytes.fetch_max(used, SeqCst);

        self.allocator.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        self.stats.cumul_dealloc_count.fetch_add(1, SeqCst);
        self.stats
            .cumul_dealloc_size
            .fetch_add(layout.size(), SeqCst);

        self.allocator.dealloc(ptr, layout);
    }
}
