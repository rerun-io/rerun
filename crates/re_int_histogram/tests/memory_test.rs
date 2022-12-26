use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

thread_local! {
    static LIVE_BYTES_IN_THREAD: AtomicUsize = AtomicUsize::new(0);
}

pub struct TrackingAllocator {
    allocator: std::alloc::System,
}

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator {
    allocator: std::alloc::System,
};

#[allow(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    #[allow(clippy::let_and_return)]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_add(layout.size(), SeqCst));

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_sub(layout.size(), SeqCst));

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}

fn live_bytes() -> usize {
    LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(SeqCst))
}

// ----------------------------------------------------------------------------

fn memory_use<R>(run: impl FnOnce() -> R) -> usize {
    let used_bytes_start = live_bytes();
    let ret = run();
    let bytes_used = live_bytes() - used_bytes_start;
    drop(ret);
    bytes_used
}

use insta::assert_debug_snapshot;

/// Number of elements
const N: i64 = 1_000_000;

#[test]
fn test_memory_use_btree() {
    use re_int_histogram::BTreeeInt64Histogram;

    fn create(num_elements: i64, spacing: i64) -> BTreeeInt64Histogram {
        let mut histogram = BTreeeInt64Histogram::default();
        for i in 0..num_elements {
            histogram.increment(i * spacing, 1);
        }
        histogram
    }

    fn bytes_per_entry(num_elements: i64, spacing: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, spacing));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "btree",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!("{:.1} B/entry, spacing: 1M", bytes_per_entry(N, 1_000_000)),
            format!("{:.1} B/entry, spacing: 2M", bytes_per_entry(N, 2_000_000)),
            format!("{:.1} B/entry, spacing: 3M", bytes_per_entry(N, 3_000_000)),
            format!("{:.1} B/entry, spacing: 5M", bytes_per_entry(N, 5_000_000)),
            format!("{:.1} B/entry, spacing: 8M", bytes_per_entry(N, 8_000_000)),
            format!(
                "{:.1} B/entry, spacing: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
}

#[test]
fn test_memory_use_tree2() {
    use re_int_histogram::tree2::IntHistogram;

    fn create(num_elements: i64, spacing: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * spacing, 1);
        }
        histogram
    }

    fn bytes_per_entry(num_elements: i64, spacing: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, spacing));
        num_bytes as f64 / num_elements as f64
    }
    assert_debug_snapshot!(
        "tree2",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!("{:.1} B/entry, spacing: 1M", bytes_per_entry(N, 1_000_000)),
            format!("{:.1} B/entry, spacing: 2M", bytes_per_entry(N, 2_000_000)),
            format!("{:.1} B/entry, spacing: 3M", bytes_per_entry(N, 3_000_000)),
            format!("{:.1} B/entry, spacing: 5M", bytes_per_entry(N, 5_000_000)),
            format!("{:.1} B/entry, spacing: 8M", bytes_per_entry(N, 8_000_000)),
            format!(
                "{:.1} B/entry, spacing: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
}

#[test]
fn test_memory_use_tree8() {
    use re_int_histogram::tree8::Int64Histogram;

    fn create(num_elements: i64, spacing: i64) -> Int64Histogram {
        let mut histogram = Int64Histogram::default();
        for i in 0..num_elements {
            histogram.increment(i * spacing, 1);
        }
        histogram
    }

    fn bytes_per_entry(num_elements: i64, spacing: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, spacing));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "tree8",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!("{:.1} B/entry, spacing: 1M", bytes_per_entry(N, 1_000_000)),
            format!("{:.1} B/entry, spacing: 2M", bytes_per_entry(N, 2_000_000)),
            format!("{:.1} B/entry, spacing: 3M", bytes_per_entry(N, 3_000_000)),
            format!("{:.1} B/entry, spacing: 5M", bytes_per_entry(N, 5_000_000)),
            format!("{:.1} B/entry, spacing: 8M", bytes_per_entry(N, 8_000_000)),
            format!(
                "{:.1} B/entry, spacing: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
}

#[test]
fn test_memory_use_tree16() {
    use re_int_histogram::tree16::IntHistogram;

    fn create(num_elements: i64, spacing: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * spacing, 1);
        }
        histogram
    }

    fn bytes_per_entry(num_elements: i64, spacing: i64) -> f64 {
        let num_bytes = memory_use(|| create(num_elements, spacing));
        num_bytes as f64 / num_elements as f64
    }

    assert_debug_snapshot!(
        "tree16",
        [
            format!("{:.1} B/entry, dense", bytes_per_entry(N, 1)),
            format!("{:.1} B/entry, spacing: 1M", bytes_per_entry(N, 1_000_000)),
            format!("{:.1} B/entry, spacing: 2M", bytes_per_entry(N, 2_000_000)),
            format!("{:.1} B/entry, spacing: 3M", bytes_per_entry(N, 3_000_000)),
            format!("{:.1} B/entry, spacing: 5M", bytes_per_entry(N, 5_000_000)),
            format!("{:.1} B/entry, spacing: 8M", bytes_per_entry(N, 8_000_000)),
            format!(
                "{:.1} B/entry, spacing: 13M",
                bytes_per_entry(N, 13_000_000)
            ),
        ]
    );
}
