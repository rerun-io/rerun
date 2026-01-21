use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

thread_local! {
    static LIVE_BYTES_IN_THREAD: AtomicUsize = const { AtomicUsize::new(0) };
}

/// Allocator that can be used in test for byte-accurate measurement of memory usage:
///
/// ```
/// #[global_allocator]
/// pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator::system();
///
/// fn size_of_thing() -> usize {
///     let num_bytes = memory_use(|| {
///         vec![0u8; 1024 * 1024]
///     });
///     assert_eq!(num_bytes, 1024 * 1024);
/// }
/// ```
pub struct TrackingAllocator {
    allocator: std::alloc::System,
}

impl TrackingAllocator {
    pub const fn system() -> Self {
        Self {
            allocator: std::alloc::System,
        }
    }

    /// Number of live bytes allocated on the current thread.
    pub fn live_bytes() -> usize {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(Relaxed))
    }

    /// Assumes all allocations are on the calling thread.
    ///
    /// The reason we use thread-local counting is so that
    /// the counting won't be confused by any other running threads (e.g. other tests).
    ///
    /// Returns `(ret, num_bytes_allocated_by_this_thread)`.
    pub fn memory_use<R>(run: impl Fn() -> R) -> (R, usize) {
        let used_bytes_start = Self::live_bytes();
        let ret = run();
        let bytes_used = Self::live_bytes() - used_bytes_start;
        (ret, bytes_used)
    }
}

#[expect(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_add(layout.size(), Relaxed));

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_sub(layout.size(), Relaxed));

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}
