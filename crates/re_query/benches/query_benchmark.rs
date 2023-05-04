#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

use itertools::Itertools;
use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Point2D, Vec3D},
    datagen::{build_frame_nr, build_some_colors, build_some_point2d, build_some_vec3d},
    entity_path, Component, DataRow, EntityPath, Index, RowId, TimeType, Timeline,
};
use re_query::query_entity_with_primary;

// ---

#[cfg(not(debug_assertions))]
const NUM_FRAMES_POINTS: u32 = 1_000;
#[cfg(not(debug_assertions))]
const NUM_POINTS: u32 = 1_000;
#[cfg(not(debug_assertions))]
const NUM_FRAMES_VECS: u32 = 10;
#[cfg(not(debug_assertions))]
const NUM_VECS: u32 = 100_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_FRAMES_POINTS: u32 = 1;
#[cfg(debug_assertions)]
const NUM_POINTS: u32 = 1;
#[cfg(debug_assertions)]
const NUM_FRAMES_VECS: u32 = 1;
#[cfg(debug_assertions)]
const NUM_VECS: u32 = 1;

criterion_group!(benches, mono_points, batch_points, batch_vecs);
criterion_main!(benches);

// --- Benchmarks ---

fn mono_points(c: &mut Criterion) {
    // Each mono point gets logged at a different path
    let paths = (0..NUM_POINTS)
        .map(move |point_idx| entity_path!("points", Index::Sequence(point_idx as _)))
        .collect_vec();
    let msgs = build_points_rows(&paths, 1);

    {
        let mut group = c.benchmark_group("arrow_mono_points");
        // Mono-insert is slow -- decrease the sample size
        group.sample_size(10);
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES_POINTS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_rows(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_mono_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let mut store = insert_rows(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_points(&mut store, &paths));
        });
    }
}

fn batch_points(c: &mut Criterion) {
    // Batch points are logged together at a single path
    let paths = [EntityPath::from("points")];
    let msgs = build_points_rows(&paths, NUM_POINTS as _);

    {
        let mut group = c.benchmark_group("arrow_batch_points");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES_POINTS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_rows(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_batch_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let mut store = insert_rows(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_points(&mut store, &paths));
        });
    }
}

fn batch_vecs(c: &mut Criterion) {
    // Batch points are logged together at a single path
    let paths = [EntityPath::from("vec")];
    let msgs = build_vecs_rows(&paths, NUM_VECS as _);

    {
        let mut group = c.benchmark_group("arrow_batch_vecs");
        group.throughput(criterion::Throughput::Elements(
            (NUM_VECS * NUM_FRAMES_VECS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_rows(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_batch_vecs");
        group.throughput(criterion::Throughput::Elements(NUM_VECS as _));
        let mut store = insert_rows(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_vecs(&mut store, &paths));
        });
    }
}

// --- Helpers ---

fn build_points_rows(paths: &[EntityPath], pts: usize) -> Vec<DataRow> {
    (0..NUM_FRAMES_POINTS)
        .flat_map(move |frame_idx| {
            paths.iter().map(move |path| {
                let mut row = DataRow::from_cells2(
                    RowId::ZERO,
                    path.clone(),
                    [build_frame_nr((frame_idx as i64).into())],
                    pts as _,
                    (build_some_point2d(pts), build_some_colors(pts)),
                );
                // NOTE: Using unsized cells will crash in debug mode, and benchmarks are run for 1 iteration,
                // in debug mode, by the standard test harness.
                if cfg!(debug_assertions) {
                    row.compute_all_size_bytes();
                }
                row
            })
        })
        .collect()
}

fn build_vecs_rows(paths: &[EntityPath], pts: usize) -> Vec<DataRow> {
    (0..NUM_FRAMES_VECS)
        .flat_map(move |frame_idx| {
            paths.iter().map(move |path| {
                let mut row = DataRow::from_cells1(
                    RowId::ZERO,
                    path.clone(),
                    [build_frame_nr((frame_idx as i64).into())],
                    pts as _,
                    build_some_vec3d(pts),
                );
                // NOTE: Using unsized cells will crash in debug mode, and benchmarks are run for 1 iteration,
                // in debug mode, by the standard test harness.
                if cfg!(debug_assertions) {
                    row.compute_all_size_bytes();
                }
                row
            })
        })
        .collect()
}

fn insert_rows<'a>(msgs: impl Iterator<Item = &'a DataRow>) -> DataStore {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());
    msgs.for_each(|row| store.insert_row(row).unwrap());
    store
}

struct SavePoint {
    _pos: Point2D,
    _color: Option<ColorRGBA>,
}

fn query_and_visit_points(store: &mut DataStore, paths: &[EntityPath]) -> Vec<SavePoint> {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::new(timeline_frame_nr, (NUM_FRAMES_POINTS as i64 / 2).into());

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    // TODO(jleibs): Add Radius once we have support for it in field_types
    for path in paths.iter() {
        query_entity_with_primary::<Point2D>(store, &query, path, &[ColorRGBA::name()])
            .and_then(|entity_view| {
                entity_view.visit2(|_: InstanceKey, pos: Point2D, color: Option<ColorRGBA>| {
                    points.push(SavePoint {
                        _pos: pos,
                        _color: color,
                    });
                })
            })
            .ok()
            .unwrap();
    }
    assert_eq!(NUM_POINTS as usize, points.len());
    points
}

struct SaveVec {
    _vec: Vec3D,
}

fn query_and_visit_vecs(store: &mut DataStore, paths: &[EntityPath]) -> Vec<SaveVec> {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::new(timeline_frame_nr, (NUM_FRAMES_POINTS as i64 / 2).into());

    let mut rects = Vec::with_capacity(NUM_VECS as _);

    for path in paths.iter() {
        query_entity_with_primary::<Vec3D>(store, &query, path, &[])
            .and_then(|entity_view| {
                entity_view.visit1(|_: InstanceKey, vec: Vec3D| {
                    rects.push(SaveVec { _vec: vec });
                })
            })
            .ok()
            .unwrap();
    }
    assert_eq!(NUM_VECS as usize, rects.len());
    rects
}
