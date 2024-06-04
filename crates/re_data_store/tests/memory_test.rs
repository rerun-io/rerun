//! Measures the memory overhead of the data store.

// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

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
/// the counting won't be confused by any other running threads (e.g. other tests).
fn memory_use<R>(run: impl Fn() -> R) -> usize {
    let used_bytes_start = live_bytes();
    let ret = run();
    let bytes_used = live_bytes() - used_bytes_start;
    drop(ret);
    bytes_used
}

// ----------------------------------------------------------------------------

use re_data_store2::{DataStore, DataStoreConfig};
use re_log_types::{DataRow, RowId, TimePoint, TimeType, Timeline};
use re_types::components::Scalar;

/// The memory overhead of storing many scalars in the store.
#[test]
fn scalar_memory_overhead() {
    re_log::setup_logging();

    const NUM_SCALARS: usize = 1024 * 1024;

    let total_mem_use = memory_use(|| {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            DataStoreConfig::default(),
        );

        for i in 0..NUM_SCALARS {
            let entity_path = re_log_types::entity_path!("scalar");
            let timepoint =
                TimePoint::default().with(Timeline::new("log_time", TimeType::Time), i as i64);
            let row = DataRow::from_cells1_sized(
                RowId::new(),
                entity_path,
                timepoint,
                vec![Scalar(i as f64)],
            )
            .unwrap();
            store.insert_row(&row).unwrap();
        }

        store
    });

    insta::assert_debug_snapshot!(
        "scalars_on_one_timeline",
        [
            format!("{NUM_SCALARS} scalars"),
            format!("{} in total", re_format::format_bytes(total_mem_use as _)),
            format!(
                "{} per row",
                re_format::format_bytes(total_mem_use as f64 / NUM_SCALARS as f64)
            ),
        ]
    );
}
