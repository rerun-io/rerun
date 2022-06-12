#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use data_store::TypePathDataStore;
use data_store::*;
use log_types::{FieldName, IndexKey, LogId};

const NUM_FRAMES: i64 = 1_000; // this can have a big impact on performance
const NUM_POINTS_PER_CAMERA: u64 = 1_000;
const TOTAL_POINTS: u64 = 2 * NUM_POINTS_PER_CAMERA;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

pub fn points_from_store<'store, Time: 'static + Clone + Ord>(
    store: &'store TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
) -> Vec<Point3<'store>> {
    let obj_type_path = TypePathComp::String("camera".into())
        / TypePathComp::Index
        / TypePathComp::String("point".into())
        / TypePathComp::Index;

    let obj_store = store.get(&obj_type_path).unwrap();

    let data_store = obj_store.get::<[f32; 3]>(&FieldName::new("pos")).unwrap();

    let mut points = vec![];
    visit_data_and_1_child(
        obj_store,
        time_query,
        data_store,
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Time(i64);

fn generate_date(individual_pos: bool, individual_radius: bool) -> TypePathDataStore<Time> {
    let mut data_store = TypePathDataStore::default();

    for frame in 0..NUM_FRAMES {
        let time_value = Time(frame);
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
                let type_path = ObjTypePath::new(vec![
                    TypePathComp::String("camera".into()),
                    TypePathComp::Index,
                    TypePathComp::String("point".into()),
                    TypePathComp::Index,
                ]);
                let mut index_path_prefix = IndexPath::default();
                index_path_prefix.push(Index::String(camera.into()));

                let batch = Arc::new(
                    (0..NUM_POINTS_PER_CAMERA)
                        .map(|pi| {
                            let pos: [f32; 3] = [1.0, 2.0, 3.0];
                            (IndexKey::new(Index::Sequence(pi)), pos)
                        })
                        .collect(),
                );

                data_store
                    .insert_batch(
                        type_path,
                        index_path_prefix,
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
                let type_path = ObjTypePath::new(vec![
                    TypePathComp::String("camera".into()),
                    TypePathComp::Index,
                    TypePathComp::String("point".into()),
                    TypePathComp::Index,
                ]);
                let mut index_path_prefix = IndexPath::default();
                index_path_prefix.push(Index::String(camera.into()));

                let batch = Arc::new(
                    (0..NUM_POINTS_PER_CAMERA)
                        .map(|pi| (IndexKey::new(Index::Sequence(pi)), 1.0_f32))
                        .collect(),
                );

                data_store
                    .insert_batch(
                        type_path,
                        index_path_prefix,
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

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("query-points-throughput");
    group.throughput(criterion::Throughput::Elements(TOTAL_POINTS as _));

    let data_store = generate_date(false, false);
    group.bench_function("batched_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, true);
    group.bench_function("individual_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(false, true);
    group.bench_function("batched_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, false);
    group.bench_function("individual_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), TOTAL_POINTS as usize);
        });
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
