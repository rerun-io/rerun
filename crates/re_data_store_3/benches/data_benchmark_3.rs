#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools as _;

use re_log_types::{obj_path, FieldName, MsgId};

use re_data_store_3::*;

const NUM_FRAMES: i64 = 1_000; // this can have a big impact on performance
const NUM_POINTS_PER_CAMERA: u64 = 1_000;
const TOTAL_POINTS: u64 = 2 * NUM_POINTS_PER_CAMERA;

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

// ----------------------------------------------------------------------------

pub fn points_from_store<'store, Time: 'static + Copy + Ord>(
    store: &'store FullStore<Time>,
    time_query: &TimeQuery<Time>,
) -> Vec<Point3<'store>> {
    let mut points = vec![];
    for (_, obj_store) in store.iter() {
        query::visit_type_data_1(
            obj_store,
            &FieldName::new("pos"),
            time_query,
            ("radius",),
            |_object_path, _msg_id: &MsgId, pos: &[f32; 3], radius: Option<&f32>| {
                points.push(Point3 {
                    pos,
                    radius: radius.copied(),
                });
            },
        );
    }
    points
}

fn obj_path_to_point(camera: &str, index: u64) -> ObjPath {
    obj_path!(
        "camera",
        Index::String(camera.into()),
        "point",
        Index::Sequence(index),
    )
}

fn obj_path_to_points(camera: &str) -> ObjPath {
    obj_path!("camera", Index::String(camera.into()), "points",)
}

fn type_path() -> ObjTypePath {
    ObjTypePath::new(vec![
        TypePathComp::String("camera".into()),
        TypePathComp::Index,
        TypePathComp::String("point".into()),
        TypePathComp::Index,
    ])
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Time(i64);

fn generate_data(individual: bool) -> FullStore<Time> {
    let mut data_store = FullStore::default();

    let indices = (0..NUM_POINTS_PER_CAMERA)
        .map(Index::Sequence)
        .collect_vec();
    let positions = vec![[1.0_f32; 3]; NUM_POINTS_PER_CAMERA as usize];
    let radii = vec![1.0_f32; NUM_POINTS_PER_CAMERA as usize];

    for frame in 0..NUM_FRAMES {
        let time_value = Time(frame);
        for camera in ["left", "right"] {
            if individual {
                for point in 0..NUM_POINTS_PER_CAMERA {
                    let obj_path = obj_path_to_point(camera, point);
                    data_store
                        .insert_individual::<[f32; 3]>(
                            obj_path.clone(),
                            FieldName::from("pos"),
                            time_value,
                            MsgId::random(),
                            [1.0, 2.0, 3.0],
                        )
                        .unwrap();

                    data_store
                        .insert_individual(
                            obj_path,
                            FieldName::from("radius"),
                            time_value,
                            MsgId::random(),
                            1.0_f32,
                        )
                        .unwrap();
                }
            } else {
                let obj_path = obj_path_to_points(camera);
                data_store
                    .insert_batch(
                        obj_path.clone(),
                        FieldName::from("pos"),
                        time_value,
                        MsgId::random(),
                        BatchOrSplat::new_batch(&indices, &positions),
                    )
                    .unwrap();

                data_store
                    .insert_batch(
                        obj_path,
                        FieldName::from("radius"),
                        time_value,
                        MsgId::random(),
                        BatchOrSplat::new_batch(&indices, &radii),
                    )
                    .unwrap();
            }
        }
    }

    data_store
}

fn create_batch_thoughput(c: &mut Criterion) {
    const NUM: usize = 100_000;
    let indices = (0..NUM).map(|pi| Index::Sequence(pi as _)).collect_vec();
    let positions = vec![[1.0_f32; 3]; NUM];

    let mut group = c.benchmark_group("create-batch-throughput");
    group.throughput(criterion::Throughput::Elements(NUM as _));

    group.bench_function("Batch::new", |b| {
        b.iter(|| BatchOrSplat::new_batch(&indices, &positions));
    });

    group.finish();
}

fn insert_batch_thoughput(c: &mut Criterion) {
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS: usize = 10_000;
    let indices = (0..NUM_POINTS)
        .map(|pi| Index::Sequence(pi as _))
        .collect_vec();
    let positions = vec![[1.0_f32; 3]; NUM_POINTS];
    let batch = BatchOrSplat::new_batch(&indices, &positions);

    let mut index_path_prefix = IndexPath::default();
    index_path_prefix.push(Index::String("left".into()));
    index_path_prefix.push(Index::Placeholder);

    let mut group = c.benchmark_group("insert-batch-throughput");
    group.throughput(criterion::Throughput::Elements(
        (NUM_POINTS * NUM_FRAMES) as _,
    ));

    group.bench_function("insert_batch", |b| {
        b.iter(|| {
            let mut data_store = FullStore::default();
            for frame in 0..NUM_FRAMES {
                let time_value = Time(frame as _);
                data_store
                    .insert_batch(
                        ObjPath::new(type_path(), index_path_prefix.clone()),
                        FieldName::from("pos"),
                        time_value,
                        MsgId::random(),
                        batch.clone(),
                    )
                    .unwrap();
            }
            data_store
        });
    });

    group.finish();
}

fn insert_individual_thoughput(c: &mut Criterion) {
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS: usize = 1000;

    let mut index_path_prefix = IndexPath::default();
    index_path_prefix.push(Index::String("left".into()));
    index_path_prefix.push(Index::Placeholder);

    let mut group = c.benchmark_group("insert-individual-throughput");
    group.throughput(criterion::Throughput::Elements(
        (NUM_POINTS * NUM_FRAMES) as _,
    ));

    group.bench_function("insert_individual", |b| {
        b.iter(|| {
            let mut data_store = FullStore::default();
            for frame in 0..NUM_FRAMES {
                let time_value = Time(frame as _);
                for point in 0..NUM_POINTS {
                    data_store
                        .insert_individual::<[f32; 3]>(
                            obj_path_to_point("left", point as _),
                            FieldName::from("pos"),
                            time_value,
                            MsgId::random(),
                            [1.0, 2.0, 3.0],
                        )
                        .unwrap();
                }
            }
            data_store
        });
    });

    group.finish();
}

fn query_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("query-points-throughput");
    group.throughput(criterion::Throughput::Elements(TOTAL_POINTS));

    let data_store = generate_data(false);
    group.bench_function("batched_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_data(true);
    group.bench_function("individual_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    create_batch_thoughput,
    insert_batch_thoughput,
    insert_individual_thoughput,
    query_throughput
);
criterion_main!(benches);
