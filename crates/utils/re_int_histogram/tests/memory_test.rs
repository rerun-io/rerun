#![cfg(feature = "testing")]

use insta::assert_debug_snapshot;
use re_byte_size::testing::TrackingAllocator;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator::system();

fn memory_use<R>(run: impl Fn() -> R) -> usize {
    TrackingAllocator::memory_use(run).1
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
