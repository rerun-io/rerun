#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};
use prototype::TypePathDataStore;
use prototype::*;

use std::sync::Arc;

const NUM_FRAMES: i64 = 1_000; // this can have a big impact on performance
const NUM_POINTS_PER_CAMERA: u64 = 1_000;
const TOTAL_POINTS: u64 = 2 * NUM_POINTS_PER_CAMERA;

fn data_path(camera: &str, index: u64, field: &str) -> DataPath {
    im::vector![
        DataPathComponent::Name("camera".into()),
        DataPathComponent::Index(Index::String(camera.into())),
        DataPathComponent::Name("point".into()),
        DataPathComponent::Index(Index::Sequence(index)),
        DataPathComponent::Name(field.into()),
    ]
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
                    let (type_path, index_path) = into_type_path(data_path(camera, point, "pos"));
                    data_store
                        .insert_individual::<[f32; 3]>(
                            type_path,
                            index_path,
                            time_value,
                            [1.0, 2.0, 3.0],
                        )
                        .unwrap();
                }
            } else {
                let type_path = im::vector![
                    TypePathComponent::Name("camera".into()),
                    TypePathComponent::Index,
                    TypePathComponent::Name("point".into()),
                    TypePathComponent::Index,
                    TypePathComponent::Name("pos".into())
                ];
                let mut index_path_prefix = IndexPathKey::default();
                index_path_prefix.push_back(Index::String(camera.into()));

                let batch = Arc::new(
                    (0..NUM_POINTS_PER_CAMERA)
                        .map(|pi| {
                            let pos: [f32; 3] = [1.0, 2.0, 3.0];
                            (IndexKey::new(Index::Sequence(pi)), pos)
                        })
                        .collect(),
                );

                data_store
                    .insert_batch(type_path, index_path_prefix, time_value, batch)
                    .unwrap();
            }

            if individual_radius {
                for point in 0..NUM_POINTS_PER_CAMERA {
                    let (type_path, index_path) =
                        into_type_path(data_path(camera, point, "radius"));
                    data_store
                        .insert_individual(type_path, index_path, time_value, 1.0_f32)
                        .unwrap();
                }
            } else {
                let type_path = im::vector![
                    TypePathComponent::Name("camera".into()),
                    TypePathComponent::Index,
                    TypePathComponent::Name("point".into()),
                    TypePathComponent::Index,
                    TypePathComponent::Name("radius".into())
                ];
                let mut index_path_prefix = IndexPathKey::default();
                index_path_prefix.push_back(Index::String(camera.into()));

                let batch = Arc::new(
                    (0..NUM_POINTS_PER_CAMERA)
                        .map(|pi| (IndexKey::new(Index::Sequence(pi)), 1.0_f32))
                        .collect(),
                );

                data_store
                    .insert_batch(type_path, index_path_prefix, time_value, batch)
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
            let scene =
                Scene3D::from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(scene.points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, true);
    group.bench_function("individual_pos_individual_radius", |b| {
        b.iter(|| {
            let scene =
                Scene3D::from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(scene.points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(false, true);
    group.bench_function("batched_pos_individual_radius", |b| {
        b.iter(|| {
            let scene =
                Scene3D::from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(scene.points.len(), TOTAL_POINTS as usize);
        });
    });

    let data_store = generate_date(true, false);
    group.bench_function("individual_pos_batched_radius", |b| {
        b.iter(|| {
            let scene =
                Scene3D::from_store(&data_store, &TimeQuery::LatestAt(Time(NUM_FRAMES / 2)));
            assert_eq!(scene.points.len(), TOTAL_POINTS as usize);
        });
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
