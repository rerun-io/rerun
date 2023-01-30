#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

use itertools::Itertools;
use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{
    component_types::{ColorRGBA, Instance, Point2D},
    datagen::{build_frame_nr, build_some_colors, build_some_point2d},
    entity_path,
    msg_bundle::{try_build_msg_bundle2, Component, MsgBundle},
    EntityPath, Index, MsgId, TimeType, Timeline,
};
use re_query::query_entity_with_primary;

// ---

#[cfg(not(debug_assertions))]
const NUM_FRAMES: u32 = 1_000;
#[cfg(not(debug_assertions))]
const NUM_POINTS: u32 = 1_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_FRAMES: u32 = 1;
#[cfg(debug_assertions)]
const NUM_POINTS: u32 = 1;

criterion_group!(benches, mono_points, batch_points);
criterion_main!(benches);

// --- Benchmarks ---

fn mono_points(c: &mut Criterion) {
    // Each mono point gets logged at a different path
    let paths = (0..NUM_POINTS)
        .into_iter()
        .map(move |point_idx| entity_path!("points", Index::Sequence(point_idx as _)))
        .collect_vec();
    let msgs = build_messages(&paths, 1);

    {
        let mut group = c.benchmark_group("arrow_mono_points");
        // Mono-insert is slow -- decrease the sample size
        group.sample_size(10);
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_messages(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_mono_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let mut store = insert_messages(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit(&mut store, &paths));
        });
    }
}

fn batch_points(c: &mut Criterion) {
    // Batch points are logged together at a single path
    let paths = [EntityPath::from("points")];
    let msgs = build_messages(&paths, NUM_POINTS as _);

    {
        let mut group = c.benchmark_group("arrow_batch_points");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_messages(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_batch_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let mut store = insert_messages(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit(&mut store, &paths));
        });
    }
}

// --- Helpers ---

fn build_messages(paths: &[EntityPath], pts: usize) -> Vec<MsgBundle> {
    (0..NUM_FRAMES)
        .into_iter()
        .flat_map(move |frame_idx| {
            paths.iter().map(move |path| {
                try_build_msg_bundle2(
                    MsgId::ZERO,
                    path.clone(),
                    [build_frame_nr((frame_idx as i64).into())],
                    (build_some_point2d(pts), build_some_colors(pts)),
                )
                .unwrap()
            })
        })
        .collect()
}

fn insert_messages<'a>(msgs: impl Iterator<Item = &'a MsgBundle>) -> DataStore {
    let mut store = DataStore::new(Instance::name(), Default::default());
    msgs.for_each(|msg_bundle| store.insert(msg_bundle).unwrap());
    store
}

struct Point {
    _pos: Point2D,
    _color: Option<ColorRGBA>,
}

fn query_and_visit(store: &mut DataStore, paths: &[EntityPath]) -> Vec<Point> {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::new(timeline_frame_nr, (NUM_FRAMES as i64 / 2).into());

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    // TODO(jleibs): Add Radius once we have support for it in field_types
    for path in paths.iter() {
        query_entity_with_primary::<Point2D>(store, &query, path, &[ColorRGBA::name()])
            .and_then(|entity_view| {
                entity_view.visit2(|_: Instance, pos: Point2D, color: Option<ColorRGBA>| {
                    points.push(Point {
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
