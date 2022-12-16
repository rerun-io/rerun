#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array, ListArray, StructArray};
use criterion::{criterion_group, criterion_main, Criterion};

use itertools::Itertools;
use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    data_types::Color,
    datagen::{build_frame_nr, build_some_colors, build_some_point2d, build_some_rects},
    field_types::{ColorRGBA, Instance, Point2D, Rect2D},
    msg_bundle::{try_build_msg_bundle2, Component, MsgBundle},
    obj_path, Index, MsgId, ObjPath, ObjPathComp, TimeType, Timeline,
};
use re_query::{query_entity_with_primary, visit_components3};

// ---

#[cfg(not(debug_assertions))]
const NUM_FRAMES: u32 = 1000;
#[cfg(not(debug_assertions))]
const NUM_POINTS: u32 = 1000;

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
        let msgs = mono_data_messages(&paths);
        let mut store = insert_messages(msgs.iter());

        {
            let mut group = c.benchmark_group("obj_mono_points");
            group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
            group.bench_function("query", |b| {
                b.iter(|| query_store(&mut store, &paths));
            });
        }

        {
            let mut group = c.benchmark_group("obj_mono_points");
            group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
            group.bench_function("join", |b| {
                b.iter(|| query_and_join(&mut store, &paths));
            });
        }

        {
            let mut group = c.benchmark_group("obj_mono_points");
            group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
            group.bench_function("visit", |b| {
                b.iter(|| query_and_visit(&mut store, &paths));
            });
        }
    }
}

criterion_group!(benches, obj_mono_points,);
criterion_main!(benches);

// --- Helpers ---

fn mono_data_messages(paths: &[ObjPath]) -> Vec<MsgBundle> {
    (0..NUM_FRAMES)
        .into_iter()
        .flat_map(move |frame_idx| {
            paths.iter().map(move |path| {
                try_build_msg_bundle2(
                    MsgId::ZERO,
                    path.clone(),
                    [build_frame_nr(frame_idx as _)],
                    (build_some_point2d(1), build_some_colors(1)),
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
    pos: Point2D,
    color: Option<ColorRGBA>,
}

fn query_store(store: &mut DataStore, paths: &[ObjPath]) {
    let time_query = TimeQuery::LatestAt((NUM_FRAMES as i64) / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    for path in paths.iter() {
        let row_indices = store
            .query(
                &timeline_query,
                path,
                Point2D::NAME,
                &[Point2D::NAME, Instance::NAME],
            )
            .unwrap_or_default();
        let mut point_results = store.get(&[Point2D::NAME, Instance::NAME], &row_indices);

        let row_indices = store
            .query(
                &timeline_query,
                path,
                Point2D::NAME,
                &[Point2D::NAME, Instance::NAME],
            )
            .unwrap_or_default();
        let mut color_results = store.get(&[ColorRGBA::NAME, Instance::NAME], &row_indices);

        let num_rects = point_results[0]
            .clone()
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray<i32>>()
            .unwrap()
            .len();

        for _ in 0..num_rects {
            points.push(Point {
                pos: Point2D { x: 0.0, y: 0.0 },
                color: Some(ColorRGBA(0)),
            });
        }
    }
    assert_eq!(NUM_POINTS as usize, points.len());
}

fn query_and_join(store: &mut DataStore, paths: &[ObjPath]) {
    let time_query = TimeQuery::LatestAt((NUM_FRAMES as i64) / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    for path in paths.iter() {
        if let Ok(df) = query_entity_with_primary(
            &store,
            &timeline_query,
            path,
            Point2D::NAME,
            &[ColorRGBA::NAME],
        ) {
            for _ in 0..df.height() {
                points.push(Point {
                    pos: Point2D { x: 0.0, y: 0.0 },
                    color: Some(ColorRGBA(0)),
                });
            }
        };
    }
    assert_eq!(NUM_POINTS as usize, points.len());
}

fn query_and_visit(store: &mut DataStore, paths: &[ObjPath]) {
    let time_query = TimeQuery::LatestAt((NUM_FRAMES as i64) / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);

    let mut points = Vec::with_capacity(NUM_POINTS as _);

    for path in paths.iter() {
        if let Ok(df) = query_entity_with_primary(
            &store,
            &timeline_query,
            path,
            Point2D::NAME,
            &[ColorRGBA::NAME],
        ) {
            visit_components3(
                &df,
                |pos: &Point2D, _instance: Option<&Instance>, color: Option<&ColorRGBA>| {
                    points.push(Point {
                        pos: pos.clone(),
                        color: color.cloned(),
                    });
                },
            );
        };
    }
    assert_eq!(NUM_POINTS as usize, points.len());
}
