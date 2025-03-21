use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

thread_local! {
    static LIVE_BYTES_IN_THREAD: AtomicUsize = const { AtomicUsize::new(0) };
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

fn live_bytes() -> usize {
    LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(Relaxed))
}

/// Assumes all allocations are on the calling thread.
///
/// The reason we use thread-local counting is so that
/// the counting won't be confused by anyt other running threads (e.g. other tests).
fn memory_use<R>(run: impl Fn() -> R) -> usize {
    let used_bytes_start = live_bytes();
    let ret = run();
    let bytes_used = live_bytes() - used_bytes_start;
    drop(ret);
    bytes_used
}

// ----------------------------------------------------------------------------

/// Baseline for performance and memory benchmarks
#[derive(Default)]
pub struct BTreeInt64Histogram {
    map: std::collections::BTreeMap<i64, u32>,
}

impl BTreeInt64Histogram {
    pub fn increment(&mut self, key: i64, inc: u32) {
        *self.map.entry(key).or_default() += inc;
    }
}

// ----------------------------------------------------------------------------

use insta::assert_debug_snapshot;

/// Number of elements
const N: i64 = 1_000_000;

#[test]
fn test_memory_use_btree() {
    use BTreeInt64Histogram;

    fn create(num_elements: i64, spacing: i64) -> BTreeInt64Histogram {
        let mut histogram = BTreeInt64Histogram::default();
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
fn test_memory_use_tree() {
    use re_int_histogram::Int64Histogram;

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
        "Int64Histogram",
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
