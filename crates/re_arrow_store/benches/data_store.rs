#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array, StructArray};
use criterion::{criterion_group, criterion_main, Criterion};

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_some_instances, build_some_rects},
    field_types::{Instance, Rect2D},
    msg_bundle::{try_build_msg_bundle2, Component as _, MsgBundle},
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

fn latest_at_batch(c: &mut Criterion) {
    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut group = c.benchmark_group("datastore/latest_at/batch/rects");
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
        let mut group = c.benchmark_group("datastore/latest_at/batch/rects");
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

fn latest_at_missing_components(c: &mut Criterion) {
    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut store = insert_messages(Instance::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/latest_at/missing_components");
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
        let mut group = c.benchmark_group("datastore/latest_at/missing_components");
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

criterion_group!(benches, latest_at_batch, latest_at_missing_components);
criterion_main!(benches);

// --- Helpers ---

fn build_messages(n: usize) -> Vec<MsgBundle> {
    (0..NUM_FRAMES)
        .into_iter()
        .map(move |frame_idx| {
            try_build_msg_bundle2(
                MsgId::ZERO,
                "rects",
                [build_frame_nr(frame_idx.into())],
                (build_some_instances(n), build_some_rects(n)),
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
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = LatestAtQuery::new(timeline_frame_nr, (NUM_FRAMES / 2).into());
    let ent_path = EntityPath::from("rects");

    let row_indices = store
        .latest_at(&timeline_query, &ent_path, primary, secondaries)
        .unwrap_or_else(|| [(); N].map(|_| None));
    store.get(secondaries, &row_indices)
}
