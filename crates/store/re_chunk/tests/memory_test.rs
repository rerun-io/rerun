//! Measures the memory overhead of the chunk store.

// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

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

fn live_bytes_local() -> usize {
    LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(Relaxed))
}

/// Returns `(ret, num_bytes_allocated_by_this_thread)`.
fn memory_use<R>(run: impl Fn() -> R) -> (R, usize) {
    let used_bytes_start_local = live_bytes_local();
    let ret = run();
    let bytes_used_local = live_bytes_local() - used_bytes_start_local;
    (ret, bytes_used_local)
}

// ----------------------------------------------------------------------------

use arrow::{
    array::{
        Array as ArrowArray, BooleanArray as ArrowBooleanArray, Int32Array as ArrowInt32Array,
        Int64Array as ArrowInt64Array, ListArray as ArrowListArray,
    },
    buffer::OffsetBuffer as ArrowOffsetBuffer,
    datatypes::Field as ArrowField,
};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_types_core::arrow_helpers::as_array_ref;

// --- concat ---

#[test]
fn concat_does_allocate() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (
        ((_unconcatenated, unconcatenated_size_bytes), (_concatenated, concatenated_size_bytes)),
        total_size_bytes,
    ) = memory_use(|| {
        let unconcatenated = memory_use(|| {
            std::iter::repeat(NUM_SCALARS as usize / 10)
                .take(10)
                .map(|_| as_array_ref(ArrowInt64Array::from((0..NUM_SCALARS / 10).collect_vec())))
                .collect_vec()
        });
        let unconcatenated_refs: Vec<&dyn ArrowArray> = unconcatenated
            .0
            .iter()
            .map(|a| &**a as &dyn ArrowArray)
            .collect_vec();

        let concatenated =
            memory_use(|| re_arrow_util::concat_arrays(&unconcatenated_refs).unwrap());

        (unconcatenated, concatenated)
    });

    eprintln!(
        "unconcatenated={} concatenated={} total={}",
        re_format::format_bytes(unconcatenated_size_bytes as _),
        re_format::format_bytes(concatenated_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(unconcatenated_size_bytes + concatenated_size_bytes <= total_size_bytes);
    assert!(unconcatenated_size_bytes as f64 >= concatenated_size_bytes as f64 * 0.95);
    assert!(unconcatenated_size_bytes as f64 <= concatenated_size_bytes as f64 * 1.05);
}

#[test]
fn concat_single_is_noop() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (
        ((unconcatenated, unconcatenated_size_bytes), (concatenated, concatenated_size_bytes)),
        total_size_bytes,
    ) = memory_use(|| {
        let unconcatenated =
            memory_use(|| as_array_ref(ArrowInt64Array::from((0..NUM_SCALARS).collect_vec())));

        let concatenated =
            memory_use(|| re_arrow_util::concat_arrays(&[&*unconcatenated.0]).unwrap());

        (unconcatenated, concatenated)
    });

    eprintln!(
        "unconcatenated={} concatenated={} total={}",
        re_format::format_bytes(unconcatenated_size_bytes as _),
        re_format::format_bytes(concatenated_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(concatenated_size_bytes < 128);
    assert!(unconcatenated_size_bytes as f64 >= total_size_bytes as f64 * 0.95);
    assert!(unconcatenated_size_bytes as f64 <= total_size_bytes as f64 * 1.05);

    {
        let unconcatenated = unconcatenated
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();
        let concatenated = concatenated
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();

        assert!(
            std::ptr::eq(
                unconcatenated.values().as_ptr_range().start,
                concatenated.values().as_ptr_range().start
            ),
            "whole thing should be a noop -- pointers should match"
        );
    }
}

// --- filter ---

#[test]
fn filter_does_allocate() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (((unfiltered, unfiltered_size_bytes), (filtered, filtered_size_bytes)), total_size_bytes) =
        memory_use(|| {
            let unfiltered = memory_use(|| {
                let scalars = ArrowInt64Array::from((0..NUM_SCALARS).collect_vec());
                ArrowListArray::new(
                    ArrowField::new("item", scalars.data_type().clone(), false).into(),
                    ArrowOffsetBuffer::from_lengths(
                        std::iter::repeat(NUM_SCALARS as usize / 10).take(10),
                    ),
                    as_array_ref(scalars),
                    None,
                )
            });

            let filter =
                ArrowBooleanArray::from((0..unfiltered.0.len()).map(|i| i % 2 == 0).collect_vec());
            let filtered = memory_use(|| re_arrow_util::filter_array(&unfiltered.0, &filter));

            (unfiltered, filtered)
        });

    eprintln!(
        "unfiltered={} filtered={} total={}",
        re_format::format_bytes(unfiltered_size_bytes as _),
        re_format::format_bytes(filtered_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(unfiltered_size_bytes + filtered_size_bytes <= total_size_bytes);
    assert!(unfiltered_size_bytes <= filtered_size_bytes * 2);

    {
        let unfiltered = unfiltered
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();
        let filtered = filtered
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();

        assert!(
            !std::ptr::eq(
                unfiltered.values().as_ptr_range().start,
                filtered.values().as_ptr_range().start
            ),
            "data should be copied -- pointers shouldn't match"
        );
    }
}

#[test]
fn filter_empty_or_full_is_noop() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (((unfiltered, unfiltered_size_bytes), (filtered, filtered_size_bytes)), total_size_bytes) =
        memory_use(|| {
            let unfiltered = memory_use(|| {
                let scalars = ArrowInt64Array::from((0..NUM_SCALARS).collect_vec());
                ArrowListArray::new(
                    ArrowField::new("item", scalars.data_type().clone(), false).into(),
                    ArrowOffsetBuffer::from_lengths(
                        std::iter::repeat(NUM_SCALARS as usize / 10).take(10),
                    ),
                    as_array_ref(scalars),
                    None,
                )
            });

            let filter = ArrowBooleanArray::from(
                std::iter::repeat(true)
                    .take(unfiltered.0.len())
                    .collect_vec(),
            );
            let filtered = memory_use(|| re_arrow_util::filter_array(&unfiltered.0, &filter));

            (unfiltered, filtered)
        });

    eprintln!(
        "unfiltered={} filtered={} total={}",
        re_format::format_bytes(unfiltered_size_bytes as _),
        re_format::format_bytes(filtered_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(
        filtered_size_bytes < 1_000,
        "filtered array should be the size of a few empty datastructures at most"
    );

    {
        let unfiltered = unfiltered
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();
        let filtered = filtered
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();

        assert!(
            std::ptr::eq(
                unfiltered.values().as_ptr_range().start,
                filtered.values().as_ptr_range().start
            ),
            "whole thing should be a noop -- pointers should match"
        );
    }
}

// --- take ---

#[test]
// TODO(cmc): That's the end goal, but it is simply impossible with `ListArray`'s encoding.
//            See `Chunk::take_array`'s doc-comment for more information.
#[should_panic = "assertion failed: untaken_size_bytes > taken_size_bytes * 10"]
fn take_does_not_allocate() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (((untaken, untaken_size_bytes), (taken, taken_size_bytes)), total_size_bytes) =
        memory_use(|| {
            let untaken = memory_use(|| {
                let scalars = ArrowInt64Array::from((0..NUM_SCALARS).collect_vec());
                ArrowListArray::new(
                    ArrowField::new("item", scalars.data_type().clone(), false).into(),
                    ArrowOffsetBuffer::from_lengths(
                        std::iter::repeat(NUM_SCALARS as usize / 10).take(10),
                    ),
                    as_array_ref(scalars),
                    None,
                )
            });

            let indices = ArrowInt32Array::from(
                (0..untaken.0.len() as i32)
                    .filter(|i| i % 2 == 0)
                    .collect_vec(),
            );
            let taken = memory_use(|| re_arrow_util::take_array(&untaken.0, &indices));

            (untaken, taken)
        });

    eprintln!(
        "untaken={} taken={} total={}",
        re_format::format_bytes(untaken_size_bytes as _),
        re_format::format_bytes(taken_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(untaken_size_bytes + taken_size_bytes <= total_size_bytes);
    assert!(untaken_size_bytes > taken_size_bytes * 10);

    {
        let untaken = untaken
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();
        let taken = taken
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();

        assert!(
            std::ptr::eq(
                untaken.values().as_ptr_range().start,
                taken.values().as_ptr_range().start
            ),
            "data shouldn't be duplicated -- pointers should match"
        );
    }
}

#[test]
fn take_empty_or_full_is_noop() {
    re_log::setup_logging();

    const NUM_SCALARS: i64 = 10_000_000;

    let (((untaken, untaken_size_bytes), (taken, taken_size_bytes)), total_size_bytes) =
        memory_use(|| {
            let untaken = memory_use(|| {
                let scalars = ArrowInt64Array::from((0..NUM_SCALARS).collect_vec());
                ArrowListArray::new(
                    ArrowField::new("item", scalars.data_type().clone(), false).into(),
                    ArrowOffsetBuffer::from_lengths(
                        std::iter::repeat(NUM_SCALARS as usize / 10).take(10),
                    ),
                    as_array_ref(scalars),
                    None,
                )
            });

            let indices = ArrowInt32Array::from((0..untaken.0.len() as i32).collect_vec());
            let taken = memory_use(|| re_arrow_util::take_array(&untaken.0, &indices));

            (untaken, taken)
        });

    eprintln!(
        "untaken={} taken={} total={}",
        re_format::format_bytes(untaken_size_bytes as _),
        re_format::format_bytes(taken_size_bytes as _),
        re_format::format_bytes(total_size_bytes as _),
    );

    assert!(
        taken_size_bytes < 1_000,
        "taken array should be the size of a few empty datastructures at most"
    );

    {
        let untaken = untaken
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();
        let taken = taken
            .values()
            .downcast_array_ref::<ArrowInt64Array>()
            .unwrap();

        assert!(
            std::ptr::eq(
                untaken.values().as_ptr_range().start,
                taken.values().as_ptr_range().start
            ),
            "whole thing should be a noop -- pointers should match"
        );
    }
}
