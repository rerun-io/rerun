//! Measures the memory overhead of the chunk store.

// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]
#![expect(clippy::cast_possible_wrap)]

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

static LIVE_BYTES_GLOBAL: AtomicUsize = AtomicUsize::new(0);

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

#[expect(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_add(layout.size(), Relaxed));
        LIVE_BYTES_GLOBAL.fetch_add(layout.size(), Relaxed);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        LIVE_BYTES_IN_THREAD.with(|bytes| bytes.fetch_sub(layout.size(), Relaxed));
        LIVE_BYTES_GLOBAL.fetch_sub(layout.size(), Relaxed);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}

fn live_bytes_local() -> usize {
    LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(Relaxed))
}

fn live_bytes_global() -> usize {
    LIVE_BYTES_GLOBAL.load(Relaxed)
}

/// Returns `(num_bytes_allocated, num_bytes_allocated_by_this_thread)`.
fn memory_use<R>(run: impl Fn() -> R) -> (usize, usize) {
    let used_bytes_start_local = live_bytes_local();
    let used_bytes_start_global = live_bytes_global();
    let ret = run();
    let bytes_used_local = live_bytes_local() - used_bytes_start_local;
    let bytes_used_global = live_bytes_global() - used_bytes_start_global;
    drop(ret);
    (bytes_used_global, bytes_used_local)
}

// ----------------------------------------------------------------------------

use re_chunk::external::crossbeam::channel::TryRecvError;
use re_chunk::{BatcherHooks, ChunkBatcher, ChunkBatcherConfig, PendingRow};
use re_chunk_store::{ChunkStore, ChunkStoreConfig};
use re_log_types::{TimePoint, Timeline};
use re_sdk_types::components::Scalar;
use re_sdk_types::{Loggable as _, SerializedComponentBatch, archetypes};

/// The memory overhead of storing many scalars in the store.
#[test]
fn scalar_memory_overhead() {
    re_log::setup_logging();

    re_log::warn!(
        "THIS TEST HAS TO ACCOUNT FOR THE MEMORY OF ALL RUNNING THREADS -- IT MUST BE RUN ON ITS OWN, WITH NO OTHER TESTS RUNNING IN PARALLEL: `cargo t --all-features -p re_chunk_store memory_tests -- scalar_memory_overhead`"
    );

    const NUM_SCALARS: usize = 1024 * 1024;

    let (total_mem_use_global, _total_mem_use_local) = memory_use(|| {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::default(),
        );

        let batcher = ChunkBatcher::new(
            ChunkBatcherConfig {
                flush_num_rows: 1000,
                ..ChunkBatcherConfig::NEVER
            },
            BatcherHooks::NONE,
        )
        .unwrap();

        for i in 0..NUM_SCALARS {
            let entity_path = re_log_types::entity_path!("scalar");
            let timepoint = TimePoint::default().with(Timeline::log_time(), i as i64);
            let scalars = Scalar::to_arrow([Scalar::from(i as f64)]).unwrap();

            let row = PendingRow::from_iter(
                timepoint,
                std::iter::once(SerializedComponentBatch::new(
                    scalars,
                    archetypes::Scalars::descriptor_scalars(),
                )),
            );

            batcher.push_row(entity_path.clone(), row);
        }

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            // eprintln!(
            //     "chunk with {} rows: {}",
            //     chunk.num_rows(),
            //     re_format::format_bytes(chunk.total_size_bytes() as _)
            // );
            _ = store.insert_chunk(&Arc::new(chunk)).unwrap();
        }

        store
    });

    insta::assert_debug_snapshot!(
        "scalars_on_one_timeline_new",
        [
            format!("{NUM_SCALARS} scalars"),
            format!(
                "{} MiB in total",
                (total_mem_use_global as f64 / 1024.0 / 1024.0).round() // Round to nearest megabyte - we get fluctuations on the kB depending on platform
            ),
            format!(
                "{} per row",
                re_format::format_bytes(total_mem_use_global as f64 / NUM_SCALARS as f64)
            ),
        ]
    );
}
