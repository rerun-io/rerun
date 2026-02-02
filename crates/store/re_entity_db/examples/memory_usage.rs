// TODO(#3408): remove unwrap()
#![expect(clippy::unwrap_used)]

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

thread_local! {
    static LIVE_BYTES_IN_THREAD: AtomicUsize = const { AtomicUsize::new(0) };
}

struct TrackingAllocator {
    allocator: std::alloc::System,
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator {
    allocator: std::alloc::System,
};

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

/// Can be used to track amount of memory allocates,
/// assuming all allocations are on the calling thread.
///
/// The reason we use thread-local counting is so that
/// the counting won't be confused by any other running threads.
/// This is not currently important, but would be important if this file were to
/// be converted into a regression test instead (tests run in parallel).
fn live_bytes() -> usize {
    LIVE_BYTES_IN_THREAD.with(|bytes| bytes.load(Relaxed))
}

// ----------------------------------------------------------------------------

use re_chunk::{Chunk, RowId};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{StoreId, StoreKind, entity_path};

fn main() {
    log_messages();
}

fn log_messages() {
    use re_log_types::{LogMsg, TimeInt, TimePoint, Timeline, build_frame_nr};

    // Note: we use Box in this function so that we also count the "static"
    // part of all the data, i.e. its `std::mem::size_of`.

    fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
        let mut bytes = vec![];
        let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
        re_log_encoding::Encoder::encode_into(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            std::iter::once(log_msg).map(Ok),
            &mut bytes,
        )
        .unwrap();
        bytes
    }

    fn decode_log_msg(mut bytes: &[u8]) -> LogMsg {
        let mut messages = re_log_encoding::Decoder::decode_lazy(&mut bytes)
            .collect::<Result<Vec<LogMsg>, _>>()
            .unwrap();
        assert!(bytes.is_empty());
        assert_eq!(messages.len(), 1);
        messages.remove(0)
    }

    // The decoded size is often smaller, presumably because all buffers
    // (e.g. Vec) have just the right capacity.
    fn size_decoded(bytes: &[u8]) -> usize {
        let used_bytes_start = live_bytes();
        let log_msg = Box::new(decode_log_msg(bytes));
        let bytes_used = live_bytes() - used_bytes_start;
        drop(log_msg);
        bytes_used
    }

    const NUM_POINTS: usize = 1_000;

    let store_id = StoreId::random(StoreKind::Recording, "test_app");
    let timeline = Timeline::new_sequence("frame_nr");
    let mut time_point = TimePoint::default();
    time_point.insert(timeline, TimeInt::ZERO);

    {
        let used_bytes_start = live_bytes();
        let entity_path = entity_path!("points");
        let used_bytes = live_bytes() - used_bytes_start;
        println!("Short entity path uses {used_bytes} bytes in RAM");
        drop(entity_path);
    }

    {
        let used_bytes_start = live_bytes();
        let chunk = Chunk::builder("points")
            .with_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::ZERO)],
                [(
                    MyPoints::descriptor_points(),
                    &MyPoint::from_iter(0..1) as _,
                )],
            )
            .build()
            .unwrap();
        let chunk_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(
            store_id.clone(),
            chunk.to_arrow_msg().unwrap(),
        ));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow payload containing a Pos2 uses {chunk_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing a Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded),
            encoded.len()
        );
    }

    {
        let used_bytes_start = live_bytes();
        let chunk = Chunk::builder("points")
            .with_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::ZERO)],
                [(
                    MyPoints::descriptor_points(),
                    &MyPoint::from_iter(0..NUM_POINTS as u32) as _,
                )],
            )
            .build()
            .unwrap();
        let chunk_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(store_id, chunk.to_arrow_msg().unwrap()));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow payload containing a Pos2 uses {chunk_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing {NUM_POINTS}x Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded),
            encoded.len()
        );
    }
}
