#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array, StructArray};
use criterion::{criterion_group, criterion_main, Criterion};

use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_some_point2d, build_some_rects},
    field_types::Rect2D,
    msg_bundle::{try_build_msg_bundle2, Component, MsgBundle},
    MsgId, ObjPath as EntityPath, TimeType, Timeline,
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
    let msgs = build_messages(NUM_RECTS as usize);
    {
        let mut group = c.benchmark_group("datastore/batch/rects");
        group.throughput(criterion::Throughput::Elements(
            (NUM_RECTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_messages(msgs.iter()));
        });
    }

    {
        let msgs = build_messages(NUM_RECTS as usize);
        let mut group = c.benchmark_group("datastore/batch/rects");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        let mut store = insert_messages(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_messages(&mut store));
        });
    }
}

criterion_group!(benches, batch_rects);
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
                (build_some_point2d(n), build_some_rects(n)),
            )
            .unwrap()
        })
        .collect()
}

fn insert_messages<'a>(msgs: impl Iterator<Item = &'a MsgBundle>) -> DataStore {
    let mut store = DataStore::default();
    msgs.for_each(|msg_bundle| store.insert(msg_bundle).unwrap());
    store
}

fn query_messages(store: &mut DataStore) -> Box<dyn Array> {
    let time_query = TimeQuery::LatestAt(NUM_FRAMES / 2);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = TimelineQuery::new(timeline_frame_nr, time_query);
    let ent_path = EntityPath::from("rects");
    let component = Rect2D::NAME;

    let row_indices = store
        .query(&timeline_query, &ent_path, component, &[component])
        .unwrap_or_default();
    let mut results = store.get(&[component], &row_indices);

    let row = std::mem::take(&mut results[0]).unwrap();
    let rects = row.as_any().downcast_ref::<StructArray>().unwrap();
    assert_eq!(NUM_RECTS as usize, rects.len());

    row
}
