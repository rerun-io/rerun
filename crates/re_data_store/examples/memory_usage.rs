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

        self.allocator.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        self.cumul_free_count.fetch_add(1, SeqCst);
        self.cumul_free_size.fetch_add(layout.size(), SeqCst);
        self.allocator.dealloc(ptr, layout);
    }
}

use itertools::Itertools as _;
use re_data_store::*;
use re_log_types::{MsgId, TimeValue};

impl TrackingAllocator {
    fn used_bytes(&self) -> usize {
        self.cumul_alloc_size.load(SeqCst) - self.cumul_free_size.load(SeqCst)
    }
}

fn obj_path(camera: u64, index: u64) -> ObjPath {
    ObjPathBuilder::new(vec![
        ObjPathComp::String("camera".into()),
        ObjPathComp::Index(Index::Sequence(camera)),
        ObjPathComp::String("point".into()),
        ObjPathComp::Index(Index::Sequence(index)),
    ])
    .into()
}

const BYTES_PER_POINT: usize = 16 + 24; // IndexPathKey + [f32; 3]

pub static GLOBAL_MUTEXT: Option<std::sync::Mutex<()>> = None;

fn tracking_points() {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

    const NUM_FRAMES: usize = 10_000;
    const OVERLAP: usize = 100;

    let mut num_points = 0;

    let mut store = TypePathDataStore::default();
    for frame in 0..NUM_FRAMES {
        for offset in 0..OVERLAP {
            store
                .insert_individual::<[f32; 3]>(
                    obj_path(0, (frame + offset) as _),
                    "pos".into(),
                    TimeValue::Sequence(frame as _),
                    MsgId::random(),
                    [1.0, 2.0, 3.0],
                )
                .unwrap();
            num_points += 1;
        }
    }

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / BYTES_PER_POINT as f32;

    println!("individual points overhead_factor: {overhead_factor}");
}

fn big_clouds() {
    let used_bytes_start = GLOBAL_ALLOCATOR.used_bytes();

    const NUM_CAMERAS: usize = 4;
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS_PER_CAMERA: usize = 1_000;

    let mut store = TypePathDataStore::default();
    let mut frame = 0;
    let mut num_points = 0;
    while frame < NUM_FRAMES {
        for camera in 0..NUM_CAMERAS {
            for point in 0..NUM_POINTS_PER_CAMERA {
                store
                    .insert_individual::<[f32; 3]>(
                        obj_path(camera as _, point as _),
                        "pos".into(),
                        TimeValue::Sequence(frame as _),
                        MsgId::random(),
                        [1.0, 2.0, 3.0],
                    )
                    .unwrap();
                num_points += 1;
            }
            frame += 1;
        }
    }

    let used_bytes = GLOBAL_ALLOCATOR.used_bytes() - used_bytes_start;

    let bytes_per_point = used_bytes as f32 / num_points as f32;
    let overhead_factor = bytes_per_point / BYTES_PER_POINT as f32;

    println!("big clouds overhead_factor: {overhead_factor}");
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

    let mut store = TypePathDataStore::default();
    let mut frame = 0;
    let mut num_points = 0;
    while frame < NUM_FRAMES {
        for camera in 0..NUM_CAMERAS {
            let batch = std::sync::Arc::new(Batch::new(&indices, &positions));
            let (obj_type_path, index_path) =
                obj_path(camera as _, 0).into_type_path_and_index_path();
            let (index_path_prefix, _) = index_path.replace_last_with_placeholder();
            store
                .insert_batch::<[f32; 3]>(
                    &ObjPath::new(obj_type_path, index_path_prefix),
                    "pos".into(),
                    TimeValue::Sequence(frame as _),
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
    let overhead_factor = bytes_per_point / BYTES_PER_POINT as f32;

    println!("big clouds batched overhead_factor: {overhead_factor}");
}

fn main() {
    tracking_points();
    big_clouds();
    big_clouds_batched();
}
