#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::BTreeMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use data_store::TypePathDataStore;
use data_store::*;
use log_types::LogId;

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
) -> BTreeMap<TypePath, Vec<Point3<'store>>> {
    let mut all = BTreeMap::default();

    for (type_path, data_store) in store.iter() {
        if let Some(data_store) = data_store.read_no_warn::<[f32; 3]>() {
            let mut point_vec = vec![];
            visit_data_and_1_sibling(
                store,
                time_query,
                type_path,
                data_store,
                ("radius",),
                |_object_path, _log_id: &LogId, pos: &[f32; 3], radius: Option<&f32>| {
                    point_vec.push(Point3 {
                        pos,
                        radius: radius.copied(),
                    });
                },
            );
            all.insert(type_path.parent(), point_vec);
        }
    }

    all
}

fn data_path(camera: &str, index: u64, field: &str) -> DataPath {
    DataPath::new(vec![
        DataPathComponent::String("camera".into()),
        DataPathComponent::Index(Index::String(camera.into())),
        DataPathComponent::String("point".into()),
        DataPathComponent::Index(Index::Sequence(index)),
        DataPathComponent::String(field.into()),
    ])
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
                            data_path(camera, point, "pos"),
                            time_value,
                            LogId::random(),
                            [1.0, 2.0, 3.0],
                        )
                        .unwrap();
                }
            } else {
                let type_path = TypePath::new(vec![
                    TypePathComponent::String("camera".into()),
                    TypePathComponent::Index,
                    TypePathComponent::String("point".into()),
                    TypePathComponent::Index,
                    TypePathComponent::String("pos".into()),
                ]);
                let mut index_path_prefix = IndexPathKey::default();
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
                            data_path(camera, point, "radius"),
                            time_value,
                            LogId::random(),
                            1.0_f32,
                        )
                        .unwrap();
                }
            } else {
                let type_path = TypePath::new(vec![
                    TypePathComponent::String("camera".into()),
                    TypePathComponent::Index,
                    TypePathComponent::String("point".into()),
                    TypePathComponent::Index,
                    TypePathComponent::String("radius".into()),
                ]);
                let mut index_path_prefix = IndexPathKey::default();
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

    let point_type_path = TypePath::new(vec![
        TypePathComponent::String("camera".into()),
        TypePathComponent::Index,
        TypePathComponent::String("point".into()),
        TypePathComponent::Index,
    ]);

    let data_store = generate_date(false, false);
    group.bench_function("batched_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), 1);
            assert_eq!(points[&point_type_path].len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, true);
    group.bench_function("individual_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), 1);
            assert_eq!(points[&point_type_path].len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(false, true);
    group.bench_function("batched_pos_individual_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), 1);
            assert_eq!(points[&point_type_path].len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, false);
    group.bench_function("individual_pos_batched_radius", |b| {
        b.iter(|| {
            let points = points_from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(points.len(), 1);
            assert_eq!(points[&point_type_path].len(), TOTAL_POINTS as usize);
        });
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
