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

/// Number of elements
const N: i64 = 1_000_000;

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

    fn bytes_per_entry(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "btree_sparse",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!(
                "{:.1} B/entry, sparseness: 1M",
                bytes_per_entry(N, 1_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 2M",
                bytes_per_entry(N, 2_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 3M",
                bytes_per_entry(N, 3_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 5M",
                bytes_per_entry(N, 5_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 8M",
                bytes_per_entry(N, 8_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
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

    fn bytes_per_entry(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "bad_sparse",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!(
                "{:.1} B/entry, sparseness: 1M",
                bytes_per_entry(N, 1_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 2M",
                bytes_per_entry(N, 2_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 3M",
                bytes_per_entry(N, 3_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 5M",
                bytes_per_entry(N, 5_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 8M",
                bytes_per_entry(N, 8_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
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

    fn bytes_per_entry(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "better_sparse",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!(
                "{:.1} B/entry, sparseness: 1M",
                bytes_per_entry(N, 1_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 2M",
                bytes_per_entry(N, 2_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 3M",
                bytes_per_entry(N, 3_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 5M",
                bytes_per_entry(N, 5_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 8M",
                bytes_per_entry(N, 8_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
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

    fn bytes_per_entry(num_elements: i64, sparseness: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, sparseness));
        num_bytes as f64 / num_elements as f64
    }
    assert_debug_snapshot!(
        "binary_sparse",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!(
                "{:.1} B/entry, sparseness: 1M",
                bytes_per_entry(N, 1_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 2M",
                bytes_per_entry(N, 2_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 3M",
                bytes_per_entry(N, 3_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 5M",
                bytes_per_entry(N, 5_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 8M",
                bytes_per_entry(N, 8_000_000)
            ),
            format!(
                "{:.1} B/entry, sparseness: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
}
