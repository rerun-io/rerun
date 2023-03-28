#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array, UnionArray};
use criterion::{criterion_group, criterion_main, Criterion};

use re_arrow_store::{DataStore, DataStoreConfig, LatestAtQuery, RangeQuery, TimeInt, TimeRange};
use re_log_types::{
    component_types::{InstanceKey, Rect2D},
    datagen::{build_frame_nr, build_some_instances, build_some_rects},
    Component as _, ComponentName, DataRow, EntityPath, MsgId, TimeType, Timeline,
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

// TODO(cmc): need additional benches for full tables

fn insert(c: &mut Criterion) {
    {
        let rows = build_rows(NUM_RECTS as usize);
        let mut group = c.benchmark_group("datastore/insert/batch/rects");
        group.throughput(criterion::Throughput::Elements(
            (NUM_RECTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_rows(Default::default(), InstanceKey::name(), rows.iter()));
        });
    }
}

fn latest_at_batch(c: &mut Criterion) {
    {
        let rows = build_rows(NUM_RECTS as usize);
        let store = insert_rows(Default::default(), InstanceKey::name(), rows.iter());
        let mut group = c.benchmark_group("datastore/latest_at/batch/rects");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("query", |b| {
            b.iter(|| {
                let results = latest_data_at(&store, Rect2D::name(), &[Rect2D::name()]);
                let rects = results[0]
                    .as_ref()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UnionArray>()
                    .unwrap();
                assert_eq!(NUM_RECTS as usize, rects.len());
            });
        });
    }
}

fn latest_at_missing_components(c: &mut Criterion) {
    // Simulate the worst possible case: many many buckets.
    let config = DataStoreConfig {
        index_bucket_size_bytes: 0,
        index_bucket_nb_rows: 0,
        ..Default::default()
    };

    {
        let msgs = build_rows(NUM_RECTS as usize);
        let store = insert_rows(config.clone(), InstanceKey::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/latest_at/missing_components");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("primary", |b| {
            b.iter(|| {
                let results =
                    latest_data_at(&store, "non_existing_component".into(), &[Rect2D::name()]);
                assert!(results[0].is_none());
            });
        });
    }

    {
        let msgs = build_rows(NUM_RECTS as usize);
        let store = insert_rows(config, InstanceKey::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/latest_at/missing_components");
        group.throughput(criterion::Throughput::Elements(NUM_RECTS as _));
        group.bench_function("secondaries", |b| {
            b.iter(|| {
                let results = latest_data_at(
                    &store,
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

fn range_batch(c: &mut Criterion) {
    {
        let msgs = build_rows(NUM_RECTS as usize);
        let store = insert_rows(Default::default(), InstanceKey::name(), msgs.iter());
        let mut group = c.benchmark_group("datastore/range/batch/rects");
        group.throughput(criterion::Throughput::Elements(
            (NUM_RECTS * NUM_FRAMES) as _,
        ));
        group.bench_function("query", |b| {
            b.iter(|| {
                let msgs = range_data(&store, [Rect2D::name()]);
                for (cur_time, (time, results)) in msgs.enumerate() {
                    let time = time.unwrap();
                    assert_eq!(cur_time as i64, time.as_i64());

                    let rects = results[0]
                        .as_ref()
                        .unwrap()
                        .as_any()
                        .downcast_ref::<UnionArray>()
                        .unwrap();
                    assert_eq!(NUM_RECTS as usize, rects.len());
                }
            });
        });
    }
}

criterion_group!(
    benches,
    insert,
    latest_at_batch,
    latest_at_missing_components,
    range_batch,
);
criterion_main!(benches);

// --- Helpers ---

fn build_rows(n: usize) -> Vec<DataRow> {
    (0..NUM_FRAMES)
        .map(move |frame_idx| {
            DataRow::from_cells2(
                MsgId::random(),
                "rects",
                [build_frame_nr(frame_idx.into())],
                n as _,
                (build_some_instances(n), build_some_rects(n)),
            )
        })
        .collect()
}

fn insert_rows<'a>(
    config: DataStoreConfig,
    cluster_key: ComponentName,
    rows: impl Iterator<Item = &'a DataRow>,
) -> DataStore {
    let mut store = DataStore::new(cluster_key, config);
    rows.for_each(|row| store.insert_row(row).unwrap());
    store
}

fn latest_data_at<const N: usize>(
    store: &DataStore,
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

fn range_data<const N: usize>(
    store: &DataStore,
    components: [ComponentName; N],
) -> impl Iterator<Item = (Option<TimeInt>, [Option<Box<dyn Array>>; N])> + '_ {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(
        timeline_frame_nr,
        TimeRange::new(0.into(), NUM_FRAMES.into()),
    );
    let ent_path = EntityPath::from("rects");

    store
        .range(&query, &ent_path, components)
        .map(move |(time, _, row_indices)| (time, store.get(&components, &row_indices)))
}
