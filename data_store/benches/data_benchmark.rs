#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use data_store::TypePathDataStore;
use data_store::*;
use itertools::Itertools;
use log_types::{FieldName, IndexKey, LogId};

const NUM_FRAMES: i64 = 1_000; // this can have a big impact on performance
const NUM_POINTS_PER_CAMERA: u64 = 1_000;
const TOTAL_POINTS: u64 = 2 * NUM_POINTS_PER_CAMERA;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

pub fn points_from_store<'store, Time: 'static + Copy + Ord>(
    store: &'store TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
) -> Vec<Point3<'store>> {
    let obj_type_path = TypePathComp::String("camera".into())
        / TypePathComp::Index
        / TypePathComp::String("point".into())
        / TypePathComp::Index;

    let obj_store = store.get(&obj_type_path).unwrap();

    let mut points = vec![];
    visit_type_data_1(
        obj_store,
        &FieldName::new("pos"),
        time_query,
        ("radius",),
        |_object_path, _log_id: &LogId, pos: &[f32; 3], radius: Option<&f32>| {
            points.push(Point3 {
                pos,
                radius: radius.copied(),
            });
        },
    );
    points
}

fn obj_path(camera: &str, index: u64) -> ObjPath {
    ObjPath::from(ObjPathBuilder::new(vec![
        ObjPathComp::String("camera".into()),
        ObjPathComp::Index(Index::String(camera.into())),
        ObjPathComp::String("point".into()),
        ObjPathComp::Index(Index::Sequence(index)),
    ]))
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

fn generate_data(individual_pos: bool, individual_radius: bool) -> TypePathDataStore<Time> {
    let mut data_store = TypePathDataStore::default();

    let type_path = type_path();

    let indices = (0..NUM_POINTS_PER_CAMERA)
        .map(|pi| IndexKey::new(Index::Sequence(pi)))
        .collect_vec();
    let positions = vec![[1.0_f32; 3]; NUM_POINTS_PER_CAMERA as usize];
    let radii = vec![1.0_f32; NUM_POINTS_PER_CAMERA as usize];

    for frame in 0..NUM_FRAMES {
        let time_value = Time(frame as _);
        for camera in ["left", "right"] {
            if individual_pos {
                for point in 0..NUM_POINTS_PER_CAMERA {
                    data_store
                        .insert_individual::<[f32; 3]>(
                            obj_path(camera, point),
                            FieldName::from("pos"),
                            time_value,
                            LogId::random(),
                            [1.0, 2.0, 3.0],
                        )
                        .unwrap();
                }
            } else {
                let mut index_path_prefix = IndexPath::default();
                index_path_prefix.push(Index::String(camera.into()));
                index_path_prefix.push(Index::Placeholder);

                let batch = Arc::new(Batch::new(&indices, &positions));

                data_store
                    .insert_batch(
                        &ObjPath::new(type_path.clone(), index_path_prefix),
                        FieldName::from("pos"),
                        time_value,
                        LogId::random(),
                        batch,
                    )
                    .unwrap();
            }

            if individual_radius {
                for point in 0..NUM_POINTS_PER_CAMERA {
                    data_store
                        .insert_individual(
                            obj_path(camera, point),
                            FieldName::from("radius"),
                            time_value,
                            LogId::random(),
                            1.0_f32,
                        )
                        .unwrap();
                }
            } else {
                let mut index_path_prefix = IndexPath::default();
                index_path_prefix.push(Index::String(camera.into()));
                index_path_prefix.push(Index::Placeholder);

                let batch = Arc::new(Batch::new(&indices, &radii));

                data_store
                    .insert_batch(
                        &ObjPath::new(type_path.clone(), index_path_prefix),
                        FieldName::from("radius"),
                        time_value,
                        LogId::random(),
                        batch,
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

    group.bench_function("IndexKey::new", |b| {
        b.iter(|| indices.iter().cloned().map(IndexKey::new).collect_vec());
    });

    let indices = indices.iter().cloned().map(IndexKey::new).collect_vec();
    group.bench_function("Batch::new", |b| {
        b.iter(|| Batch::new(&indices, &positions));
    });

    group.finish();
}

fn insert_batch_thoughput(c: &mut Criterion) {
    const NUM_FRAMES: usize = 100;
    const NUM_POINTS: usize = 10_000;
    let indices = (0..NUM_POINTS)
        .map(|pi| IndexKey::new(Index::Sequence(pi as _)))
        .collect_vec();
    let positions = vec![[1.0_f32; 3]; NUM_POINTS];
    let batch = std::sync::Arc::new(Batch::new(&indices, &positions));

    let mut index_path_prefix = IndexPath::default();
    index_path_prefix.push(Index::String("left".into()));
    index_path_prefix.push(Index::Placeholder);

    let mut group = c.benchmark_group("insert-batch-throughput");
    group.throughput(criterion::Throughput::Elements(
        (NUM_POINTS * NUM_FRAMES) as _,
    ));

    group.bench_function("insert_batch", |b| {
        b.iter(|| {
            let mut data_store = TypePathDataStore::default();
            for frame in 0..NUM_FRAMES {
                let time_value = Time(frame as _);
                data_store
                    .insert_batch(
                        &ObjPath::new(type_path(), index_path_prefix.clone()),
                        FieldName::from("pos"),
                        time_value,
                        LogId::random(),
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
            let mut data_store = TypePathDataStore::default();
            for frame in 0..NUM_FRAMES {
                let time_value = Time(frame as _);
                for point in 0..NUM_POINTS {
                    data_store
                        .insert_individual::<[f32; 3]>(
                            obj_path("left", point as _),
                            FieldName::from("pos"),
                            time_value,
                            LogId::random(),
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
    group.throughput(criterion::Throughput::Elements(TOTAL_POINTS as _));

    let data_store = generate_data(false, false);
    group.bench_function("batched_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_data(true, true);
    group.bench_function("individual_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_data(false, true);
    group.bench_function("batched_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_data(true, false);
    group.bench_function("individual_pos_batched_radius", |b| {
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
