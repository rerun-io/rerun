use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

thread_local! {
    static LIVE_BYTES_IN_THREAD: AtomicUsize = AtomicUsize::new(0);
}

struct TrackingAllocator {
    allocator: std::alloc::System,
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator {
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

use re_log_types::{entity_path, DataRow, RecordingId, RecordingType, RowId};

fn main() {
    log_messages();
}

fn log_messages() {
    use re_log_types::{
        datagen::{build_frame_nr, build_some_point2d},
        LogMsg, TimeInt, TimePoint, Timeline,
    };

    // Note: we use Box in this function so that we also count the "static"
    // part of all the data, i.e. its `std::mem::size_of`.

    fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
        let mut bytes = vec![];
        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
        re_log_encoding::encoder::encode(encoding_options, std::iter::once(log_msg), &mut bytes)
            .unwrap();
        bytes
    }

    fn decode_log_msg(mut bytes: &[u8]) -> LogMsg {
        let mut messages = re_log_encoding::decoder::Decoder::new(&mut bytes)
            .unwrap()
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

    let recording_id = RecordingId::random(RecordingType::Data);
    let timeline = Timeline::new_sequence("frame_nr");
    let mut time_point = TimePoint::default();
    time_point.insert(timeline, TimeInt::from(0));

    {
        let used_bytes_start = live_bytes();
        let entity_path = entity_path!("points");
        let used_bytes = live_bytes() - used_bytes_start;
        println!("Short EntityPath uses {used_bytes} bytes in RAM");
        drop(entity_path);
    }

    {
        let used_bytes_start = live_bytes();
        let table = Box::new(
            DataRow::from_cells1(
                RowId::random(),
                entity_path!("points"),
                [build_frame_nr(0.into())],
                1,
                build_some_point2d(1),
            )
            .into_table(),
        );
        let table_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(
            recording_id.clone(),
            table.to_arrow_msg().unwrap(),
        ));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow payload containing a Pos2 uses {table_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing a Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }

    {
        let used_bytes_start = live_bytes();
        let table = Box::new(
            DataRow::from_cells1(
                RowId::random(),
                entity_path!("points"),
                [build_frame_nr(0.into())],
                NUM_POINTS as _,
                build_some_point2d(NUM_POINTS),
            )
            .into_table(),
        );
        let table_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(
            recording_id,
            table.to_arrow_msg().unwrap(),
        ));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow payload containing a Pos2 uses {table_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing {NUM_POINTS}x Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }
}
