use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use re_data_store::log_db::collect_datatypes;
use re_log_types::{
    datagen::{build_frame_nr, build_some_point2d},
    DataCell, LogMsg, TimeInt, TimePoint, Timeline,
};

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

use re_log_types::{entity_path, DataRow, DataTable, RecordingId, RowId, TableId};

fn main() {
    log_messages();
}

fn log_messages() {
    // Note: we use Box in this function so that we also count the "static"
    // part of all the data, i.e. its `std::mem::size_of`.

    fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
        let mut bytes = vec![];
        re_log_encoding::encoder::encode(std::iter::once(log_msg), &mut bytes).unwrap();
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

    const NUM_ROWS: usize = 100_000;
    const NUM_POINTS: usize = 1_000;

    let recording_id = RecordingId::random();
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

    fn arrow_payload(recording_id: RecordingId, num_rows: usize, num_points: usize, packed: bool) {
        println!("--- {num_rows} rows each containing {num_points} points (packed={packed}) ---");
        let used_bytes_start = live_bytes();
        let table = Box::new(create_table(num_rows, num_points, packed));
        let table_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(
            recording_id,
            table.to_arrow_msg().unwrap(),
        ));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!(
            "Arrow payload containing {num_points}x Pos2 uses {} bytes in RAM",
            re_format::format_bytes(table_bytes as _)
        );
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing {num_points}x Pos2 uses {}-{} bytes in RAM, and {} bytes encoded",
            re_format::format_bytes(size_decoded(&encoded) as _),
            re_format::format_bytes(log_msg_bytes as _),
            re_format::format_bytes(encoded.len() as _),
        );
        println!();
    }

    let num_rows = [1, NUM_ROWS];
    let num_points = [1, NUM_POINTS];
    let packed = [false, true];

    for (num_rows, num_points, packed) in num_rows
        .into_iter()
        .flat_map(|num_row| std::iter::repeat(num_row).zip(num_points))
        .flat_map(|num_row| std::iter::repeat(num_row).zip(packed))
        .map(|((a, b), c)| (a, b, c))
    {
        arrow_payload(recording_id, num_rows, num_points, packed);
    }
}

fn create_table(num_rows: usize, num_points: usize, packed: bool) -> DataTable {
    let rows = (0..num_rows).map(|i| {
        DataRow::from_cells1(
            RowId::random(),
            entity_path!("points"),
            [build_frame_nr((i as i64).into())],
            num_points as _,
            build_some_point2d(num_points),
        )
    });
    let mut table = DataTable::from_rows(TableId::random(), rows);

    // Do a serialization roundtrip to pack everything in contiguous memory.
    if packed {
        let (schema, columns) = table.serialize().unwrap();

        let mut datatypes = Default::default();
        for column in columns.arrays() {
            collect_datatypes(&mut datatypes, &**column);
        }

        table = DataTable::deserialize(TableId::ZERO, &schema, columns, Some(&datatypes)).unwrap();
    }

    table
}
