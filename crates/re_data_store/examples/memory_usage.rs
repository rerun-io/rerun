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

use re_data_store::{BatchOrSplat, Index, ObjPath, TimelineStore};
use re_log_types::{obj_path, MsgId};

use itertools::Itertools as _;

fn main() {
    tracking_points();
    big_clouds();
    big_clouds_batched();
    big_clouds_sequential_batched();
    log_messages();
}

fn obj_path_mono(camera: u64, index: u64) -> ObjPath {
    obj_path!(
        "camera",
        Index::Sequence(camera),
        "point",
        Index::Sequence(index),
    )
}

fn obj_path_batch(camera: u64) -> ObjPath {
    obj_path!("camera", Index::Sequence(camera), "points",)
}

const OPTIMAL_BYTES_PER_POINT: usize = 3 * std::mem::size_of::<f32>(); // [f32; 3]

pub static GLOBAL_MUTEXT: Option<std::sync::Mutex<()>> = None;

fn tracking_points() {
    let used_bytes_start = live_bytes();

    const NUM_FRAMES: usize = 10_000;
    const OVERLAP: usize = 100;

    let mut num_points = 0;

    let mut store = TimelineStore::default();
    for frame in 0..NUM_FRAMES {
        for offset in 0..OVERLAP {
            store
                .insert_mono::<[f32; 3]>(
                    obj_path_mono(0, (frame + offset) as _),
                    "pos".into(),
                    frame,
                    MsgId::random(),
                    Some([1.0, 2.0, 3.0]),
                )
                .unwrap();
            num_points += 1;
        }
    }

    let used_bytes = live_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // NOTE: we are storing history for each point, so we will never get to OPTIMAL_BYTES_PER_POINT.
    println!(
        "individual points overhead_factor: {overhead_factor} (should ideally be just above 1)"
    );
}

fn big_clouds() {
    let used_bytes_start = live_bytes();

    const NUM_CAMERAS: usize = 4;
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS_PER_CAMERA: usize = 1_000;

    let mut store = TimelineStore::default();
    let mut frame = 0;
    let mut num_points = 0;
    while frame < NUM_FRAMES {
        for camera in 0..NUM_CAMERAS {
            for point in 0..NUM_POINTS_PER_CAMERA {
                store
                    .insert_mono::<[f32; 3]>(
                        obj_path_mono(camera as _, point as _),
                        "pos".into(),
                        frame,
                        MsgId::random(),
                        Some([1.0, 2.0, 3.0]),
                    )
                    .unwrap();
                num_points += 1;
            }
            frame += 1;
        }
    }

    let used_bytes = live_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // NOTE: we are storing history for each point, so we will never get to OPTIMAL_BYTES_PER_POINT.
    println!("big clouds overhead_factor: {overhead_factor} (should ideally be just above 1)");
}

fn big_clouds_batched() {
    let used_bytes_start = live_bytes();

    const NUM_CAMERAS: usize = 4;
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS_PER_CAMERA: usize = 1_000;

    let indices = (0..NUM_POINTS_PER_CAMERA)
        .map(|i| Index::Sequence(i as _))
        .collect_vec();
    let point: [f32; 3] = [1.0, 2.0, 3.0];
    let positions = vec![point; NUM_POINTS_PER_CAMERA];

    let mut store = TimelineStore::default();
    let mut frame = 0;
    let mut num_points = 0;
    while frame < NUM_FRAMES {
        for camera in 0..NUM_CAMERAS {
            let batch = BatchOrSplat::new_batch(&indices, &positions).unwrap();
            store
                .insert_batch::<[f32; 3]>(
                    obj_path_batch(camera as _),
                    "pos".into(),
                    frame,
                    MsgId::random(),
                    batch,
                )
                .unwrap();

            num_points += NUM_POINTS_PER_CAMERA;

            frame += 1;
        }
    }

    let used_bytes = live_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // Since we are only storing history for the entire batch, we should be able to approach OPTIMAL_BYTES_PER_POINT.
    println!(
        "big clouds batched overhead_factor: {overhead_factor} (should ideally be just above 1)"
    );
}

fn big_clouds_sequential_batched() {
    let used_bytes_start = live_bytes();

    const NUM_CAMERAS: usize = 4;
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS_PER_CAMERA: usize = 1_000;

    let point: [f32; 3] = [1.0, 2.0, 3.0];
    let positions = vec![point; NUM_POINTS_PER_CAMERA];

    let mut store = TimelineStore::default();
    let mut frame = 0;
    let mut num_points = 0;
    while frame < NUM_FRAMES {
        for camera in 0..NUM_CAMERAS {
            let batch = BatchOrSplat::new_sequential_batch(&positions).unwrap();
            store
                .insert_batch::<[f32; 3]>(
                    obj_path_batch(camera as _),
                    "pos".into(),
                    frame,
                    MsgId::random(),
                    batch,
                )
                .unwrap();

            num_points += NUM_POINTS_PER_CAMERA;

            frame += 1;
        }
    }

    let used_bytes = live_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // Since we are only storing history for the entire batch, we should be able to approach OPTIMAL_BYTES_PER_POINT.
    println!("big clouds sequential batched overhead_factor: {overhead_factor} (should ideally be just above 1)");
}

fn log_messages() {
    use re_log_types::{
        datagen::{build_frame_nr, build_some_point2d},
        msg_bundle::try_build_msg_bundle1,
        ArrowMsg, BatchIndex, Data, DataMsg, DataPath, DataVec, FieldName, LogMsg, LoggedData,
        TimeInt, TimePoint, Timeline,
    };

    // Note: we use Box in this function so that we also count the "static"
    // part of all the data, i.e. its `std::mem::size_of`.

    fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
        let mut bytes = vec![];
        re_log_types::encoding::encode(std::iter::once(log_msg), &mut bytes).unwrap();
        bytes
    }

    fn decode_log_msg(mut bytes: &[u8]) -> LogMsg {
        let mut messages = re_log_types::encoding::Decoder::new(&mut bytes)
            .unwrap()
            .collect::<anyhow::Result<Vec<LogMsg>>>()
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

    const POS: [f32; 2] = [2.0, 3.0];
    const NUM_POINTS: usize = 1_000;

    let timeline = Timeline::new_sequence("frame_nr");
    let pos_field_name = FieldName::from("pos");
    let mut time_point = TimePoint::default();
    time_point.insert(timeline, TimeInt::from(0));

    {
        let used_bytes_start = live_bytes();
        let obj_path = obj_path!("points");
        let used_bytes = live_bytes() - used_bytes_start;
        println!("Short ObjPath uses {used_bytes} bytes in RAM");
        drop(obj_path);
    }

    {
        let used_bytes_start = live_bytes();
        let log_msg = Box::new(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path!("points"), pos_field_name),
            data: Data::Vec2(POS).into(),
        }));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Classic LogMsg containing a Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }

    {
        let used_bytes_start = live_bytes();
        let msg_bundle = Box::new(
            try_build_msg_bundle1(
                MsgId::random(),
                obj_path!("points"),
                [build_frame_nr(0.into())],
                build_some_point2d(1),
            )
            .unwrap(),
        );
        let msg_bundle_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(ArrowMsg::try_from(*msg_bundle).unwrap()));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow MsgBundle containing a Pos2 uses {msg_bundle_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing a Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }

    {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();

        let used_bytes_start = live_bytes();
        let log_msg = Box::new(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path!("points"), pos_field_name),
            data: LoggedData::Batch {
                indices: BatchIndex::SequentialIndex(NUM_POINTS),
                data: DataVec::Vec2(
                    (0..NUM_POINTS)
                        .map(|_| [rng.gen_range(0.0..10.0), rng.gen_range(0.0..10.0)])
                        .collect(),
                ),
            },
        }));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Classic LogMsg containing {NUM_POINTS}x Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }

    {
        let used_bytes_start = live_bytes();
        let msg_bundle = Box::new(
            try_build_msg_bundle1(
                MsgId::random(),
                obj_path!("points"),
                [build_frame_nr(0.into())],
                build_some_point2d(NUM_POINTS),
            )
            .unwrap(),
        );
        let msg_bundle_bytes = live_bytes() - used_bytes_start;
        let log_msg = Box::new(LogMsg::ArrowMsg(ArrowMsg::try_from(*msg_bundle).unwrap()));
        let log_msg_bytes = live_bytes() - used_bytes_start;
        println!("Arrow MsgBundle containing a Pos2 uses {msg_bundle_bytes} bytes in RAM");
        let encoded = encode_log_msg(&log_msg);
        println!(
            "Arrow LogMsg containing {NUM_POINTS}x Pos2 uses {}-{log_msg_bytes} bytes in RAM, and {} bytes encoded",
            size_decoded(&encoded), encoded.len()
        );
    }
}
