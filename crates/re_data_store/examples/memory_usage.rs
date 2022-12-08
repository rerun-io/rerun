use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

pub struct TrackingAllocator {
    allocator: mimalloc::MiMalloc,

    cumul_alloc_count: AtomicUsize,
    cumul_alloc_size: AtomicUsize,
    cumul_free_count: AtomicUsize,
    cumul_free_size: AtomicUsize,

    high_water_mark_bytes: AtomicUsize,
}

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator {
    allocator: mimalloc::MiMalloc,

    cumul_alloc_count: AtomicUsize::new(0),
    cumul_alloc_size: AtomicUsize::new(0),
    cumul_free_count: AtomicUsize::new(0),
    cumul_free_size: AtomicUsize::new(0),

    high_water_mark_bytes: AtomicUsize::new(0),
};

#[allow(unsafe_code)]
// SAFETY:
// We just do book-keeping and then let another allocator do all the actual work.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    #[allow(clippy::let_and_return)]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        self.cumul_alloc_count.fetch_add(1, SeqCst);
        self.cumul_alloc_size.fetch_add(layout.size(), SeqCst);

        let used = self.used_bytes();
        self.high_water_mark_bytes
            .store(self.high_water_mark_bytes.load(SeqCst).max(used), SeqCst);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        self.cumul_free_count.fetch_add(1, SeqCst);
        self.cumul_free_size.fetch_add(layout.size(), SeqCst);

        // SAFETY:
        // Just deferring
        unsafe { self.allocator.dealloc(ptr, layout) };
    }
}

use itertools::Itertools as _;
use re_data_store::{BatchOrSplat, Index, ObjPath, ObjPathComp, TimelineStore};
use re_log_types::{obj_path, MsgId};

impl TrackingAllocator {
    fn used_bytes(&self) -> usize {
        self.cumul_alloc_size.load(SeqCst) - self.cumul_free_size.load(SeqCst)
    }
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
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

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

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // NOTE: we are storing history for each point, so we will never get to OPTIMAL_BYTES_PER_POINT.
    println!(
        "individual points overhead_factor: {overhead_factor} (should ideally be just above 1)"
    );
}

fn big_clouds() {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

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

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // NOTE: we are storing history for each point, so we will never get to OPTIMAL_BYTES_PER_POINT.
    println!("big clouds overhead_factor: {overhead_factor} (should ideally be just above 1)");
}

fn big_clouds_batched() {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

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

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // Since we are only storing history for the entire batch, we should be able to approach OPTIMAL_BYTES_PER_POINT.
    println!(
        "big clouds batched overhead_factor: {overhead_factor} (should ideally be just above 1)"
    );
}

fn big_clouds_sequential_batched() {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

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

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / OPTIMAL_BYTES_PER_POINT as f32;

    // Since we are only storing history for the entire batch, we should be able to approach OPTIMAL_BYTES_PER_POINT.
    println!("big clouds sequential batched overhead_factor: {overhead_factor} (should ideally be just above 1)");
}

fn main() {
    tracking_points();
    big_clouds();
    big_clouds_batched();
    big_clouds_sequential_batched();
}
