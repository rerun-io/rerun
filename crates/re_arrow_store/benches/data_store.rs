#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array, StructArray};
use criterion::{criterion_group, criterion_main, Criterion};

use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_some_point2d, build_some_rects},
    field_types::{Instance, Rect2D},
    msg_bundle::{try_build_msg_bundle2, try_build_msg_bundle3, Component as _, MsgBundle},
    ComponentName, MsgId, ObjPath as EntityPath, TimeType, Timeline,
};

// ---

#[cfg(not(debug_assertions))]
const NUM_FRAMES: i64 = 100;
#[cfg(not(debug_assertions))]
const NUM_RECTS: i64 = 100;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_FRAMES: i64 = 1;
#[cfg(debug_assertions)]
const NUM_RECTS: i64 = 1;

// --- Benchmarks ---

fn batch_rects(c: &mut Criterion) {
    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut group = c.benchmark_group("datastore/batch/rects");
        group.throughput(criterion::Throughput::Elements(
            (NUM_RECTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_messages(Instance::name(), msgs.iter()));
        });
    }

    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut store = insert_messages(Instance::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/batch/rects");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("query", |b| {
            b.iter(|| {
                let results = query_messages(&mut store, Rect2D::name(), &[Rect2D::name()]);
                let rects = results[0]
                    .as_ref()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .unwrap();
                assert_eq!(NUM_RECTS as usize, rects.len());
            });
        });
    }
}

fn missing_components(c: &mut Criterion) {
    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut store = insert_messages(Instance::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/missing_components");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("primary", |b| {
            b.iter(|| {
                let results = query_messages(
                    &mut store,
                    "non_existing_component".into(),
                    &[Rect2D::name()],
                );
                assert!(results[0].is_none());
            });
        });
    }

    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut store = insert_messages(Instance::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/missing_components");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("secondaries", |b| {
            b.iter(|| {
                let results = query_messages(
                    &mut store,
                    Rect2D::name(),
                    &[
                        "non_existing_component1".into(),
                        "non_existing_component2".into(),
                        "non_existing_component3".into(),
                    ],
                );
                assert!(results[0].is_none());
                assert!(results[1].is_none());
                assert!(results[2].is_none());
            });
        });
    }
}

criterion_group!(benches, batch_rects, missing_components);
criterion_main!(benches);

// --- Helpers ---

fn build_messages(n: usize) -> Vec<MsgBundle> {
    (0..NUM_FRAMES)
        .into_iter()
        .map(move |frame_idx| {
            try_build_msg_bundle2(
                MsgId::ZERO,
                "rects",
                [build_frame_nr(frame_idx)],
                (build_instances(n), build_some_rects(n)),
            )
            .unwrap()
        })
        .collect()
}

fn insert_messages<'a>(
    cluster_key: ComponentName,
    msgs: impl Iterator<Item = &'a MsgBundle>,
) -> DataStore {
    let mut store = DataStore::new(cluster_key, Default::default());
    msgs.for_each(|msg_bundle| store.insert(msg_bundle).unwrap());
    store
}

fn query_messages<const N: usize>(
    store: &mut DataStore,
    primary: ComponentName,
    secondaries: &[ComponentName; N],
) -> [Option<Box<dyn Array>>; N] {
    let time_query = TimeQuery::LatestAt(NUM_FRAMES / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);
    let ent_path = EntityPath::from("rects");

    let row_indices = store
        .query(&timeline_query, &ent_path, primary, secondaries)
        .unwrap_or_else(|| [(); N].map(|_| None));
    store.get(secondaries, &row_indices)
}
