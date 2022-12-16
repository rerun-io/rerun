#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

use itertools::Itertools;
use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_some_colors, build_some_point2d},
    field_types::{ColorRGBA, Instance, Point2D},
    msg_bundle::{try_build_msg_bundle2, Component, MsgBundle},
    obj_path, Index, MsgId, ObjPath, ObjPathComp, TimeType, Timeline,
};
use re_query::{query_entity_with_primary, visit_components3};

// ---

#[cfg(not(debug_assertions))]
const NUM_FRAMES: u32 = 100;
#[cfg(not(debug_assertions))]
const NUM_POINTS: u32 = 100;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_FRAMES: u32 = 1;
#[cfg(debug_assertions)]
const NUM_POINTS: u32 = 1;

// --- Benchmarks ---

fn obj_mono_points(c: &mut Criterion) {
    {
        // Each mono point gets logged at a different path
        let paths = (0..NUM_POINTS)
            .into_iter()
            .map(move |point_idx| obj_path!("points", Index::Sequence(point_idx as _)))
            .collect_vec();
        let msgs = build_messages(&paths, 1);

        {
            let mut group = c.benchmark_group("arrow_mono_points");
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
}

fn obj_batch_points(c: &mut Criterion) {
    {
        // Each mono point gets logged at a different path
        let paths = [ObjPath::from("points")];
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
}

criterion_group!(benches, obj_mono_points, obj_batch_points);
criterion_main!(benches);

// --- Helpers ---

fn build_messages(paths: &[ObjPath], pts: usize) -> Vec<MsgBundle> {
    (0..NUM_FRAMES)
        .into_iter()
        .flat_map(move |frame_idx| {
            paths.iter().map(move |path| {
                try_build_msg_bundle2(
                    MsgId::ZERO,
                    path.clone(),
                    [build_frame_nr(frame_idx as _)],
                    (build_some_point2d(pts), build_some_colors(pts)),
                )
                .unwrap()
            })
        })
        .collect()
}

fn insert_messages<'a>(msgs: impl Iterator<Item = &'a MsgBundle>) -> DataStore {
    let mut store = DataStore::default();
    msgs.for_each(|msg_bundle| store.insert(msg_bundle).unwrap());
    store
}

struct Point {
    _pos: Point2D,
    _color: Option<ColorRGBA>,
}

fn query_and_visit(store: &mut DataStore, paths: &[ObjPath]) {
    let time_query = TimeQuery::LatestAt((NUM_FRAMES as i64) / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    for path in paths.iter() {
        if let Ok(df) = query_entity_with_primary(
            store,
            &timeline_query,
            path,
            Point2D::NAME,
            &[ColorRGBA::NAME],
        ) {
            visit_components3(
                &df,
                |pos: &Point2D, _instance: Option<&Instance>, color: Option<&ColorRGBA>| {
                    points.push(Point {
                        _pos: pos.clone(),
                        _color: color.cloned(),
                    });
                },
            );
        };
    }
    assert_eq!(NUM_POINTS as usize, points.len());
}
