use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

pub struct TrackingAllocator {
    allocator: std::alloc::System,
    bytes_used: AtomicUsize, // TODO: thread-local
}

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator {
    allocator: std::alloc::System,
    bytes_used: AtomicUsize::new(0),
};

#[allow(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    #[allow(clippy::let_and_return)]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        self.bytes_used.fetch_add(layout.size(), SeqCst);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        self.bytes_used.fetch_sub(layout.size(), SeqCst);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}

impl TrackingAllocator {
    fn used_bytes(&self) -> usize {
        self.bytes_used.load(SeqCst)
    }
}

// ----------------------------------------------------------------------------

fn memory_use<R>(run: impl FnOnce() -> R) -> usize {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();
    let ret = run();
    let bytes_used = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;
    drop(ret);
    bytes_used
}

use insta::assert_debug_snapshot;

#[test]
fn test_memory_use_btree() {
    use re_int_histogram::BTreeeIntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> BTreeeIntHistogram {
        let mut histogram = BTreeeIntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    fn bytes_per_element(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!("btree_dense", bytes_per_element(1_000_000, 1));

    assert_debug_snapshot!("btree_sparse", bytes_per_element(1_000_000, 1_000_000));
}

#[test]
fn test_memory_use_bad() {
    use re_int_histogram::bad::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    fn bytes_per_element(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!("bad_dense", bytes_per_element(1_000_000, 1));

    assert_debug_snapshot!("bad_sparse", bytes_per_element(1_000_000, 1_000_000));
}

#[test]
fn test_memory_use_better() {
    use re_int_histogram::better::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    fn bytes_per_element(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!("better_dense", bytes_per_element(1_000_000, 1));

    assert_debug_snapshot!("better_sparse", bytes_per_element(1_000_000, 1_000_000));
}

#[test]
fn test_memory_use_binary() {
    use re_int_histogram::binary::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    fn bytes_per_element(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!("binary_dense", bytes_per_element(1_000_000, 1));

    assert_debug_snapshot!("binary_sparse", bytes_per_element(1_000_000, 1_000_000));
}
