#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::UnionArray;
use criterion::{criterion_group, criterion_main, Criterion};

use re_arrow_store::{
    DataStore, DataStoreConfig, GarbageCollectionTarget, LatestAtQuery, RangeQuery, TimeInt,
    TimeRange,
};
use re_log_types::{
    component_types::{InstanceKey, Rect2D},
    datagen::{build_frame_nr, build_some_instances, build_some_rects},
    Component as _, ComponentName, DataCell, DataRow, DataTable, EntityPath, RowId, TableId,
    TimeType, Timeline,
};

criterion_group!(benches, insert, latest_at, latest_at_missing, range, gc);
criterion_main!(benches);

// ---

#[cfg(not(debug_assertions))]
const NUM_ROWS: i64 = 1_000;
#[cfg(not(debug_assertions))]
const NUM_INSTANCES: i64 = 1_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_ROWS: i64 = 1;
#[cfg(debug_assertions)]
const NUM_INSTANCES: i64 = 1;

fn packed() -> &'static [bool] {
    #[cfg(feature = "core_benchmarks_only")]
    {
        &[false]
    }
    #[cfg(not(feature = "core_benchmarks_only"))]
    {
        &[false, true]
    }
}

fn num_rows_per_bucket() -> &'static [u64] {
    #[cfg(feature = "core_benchmarks_only")]
    {
        &[]
    }
    #[cfg(not(feature = "core_benchmarks_only"))]
    {
        &[0, 2, 32, 2048]
    }
}

// --- Benchmarks ---

fn insert(c: &mut Criterion) {
    for &packed in packed() {
        let mut group = c.benchmark_group(format!(
            "datastore/num_rows={NUM_ROWS}/num_instances={NUM_INSTANCES}/packed={packed}/insert"
        ));
        group.throughput(criterion::Throughput::Elements(
            (NUM_INSTANCES * NUM_ROWS) as _,
        ));

        let table = build_table(NUM_INSTANCES as usize, packed);

        // Default config
        group.bench_function("default", |b| {
            b.iter(|| insert_table(Default::default(), InstanceKey::name(), &table));
        });

        // Emulate more or less bucket
        for &num_rows_per_bucket in num_rows_per_bucket() {
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    insert_table(
                        DataStoreConfig {
                            indexed_bucket_num_rows: num_rows_per_bucket,
                            ..Default::default()
                        },
                        InstanceKey::name(),
                        &table,
                    )
                });
            });
        }
    }
}

fn latest_at(c: &mut Criterion) {
    for &packed in packed() {
        let mut group = c.benchmark_group(format!(
            "datastore/num_rows={NUM_ROWS}/num_instances={NUM_INSTANCES}/packed={packed}/latest_at"
        ));
        group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

        let table = build_table(NUM_INSTANCES as usize, packed);

        // Default config
        group.bench_function("default", |b| {
            let store = insert_table(Default::default(), InstanceKey::name(), &table);
            b.iter(|| {
                let cells = latest_data_at(&store, Rect2D::name(), &[Rect2D::name()]);
                let rects = cells[0]
                    .as_ref()
                    .unwrap()
                    .as_arrow_ref()
                    .as_any()
                    .downcast_ref::<UnionArray>()
                    .unwrap();
                assert_eq!(NUM_INSTANCES as usize, rects.len());
            });
        });

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            let store = insert_table(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                &table,
            );
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let cells = latest_data_at(&store, Rect2D::name(), &[Rect2D::name()]);
                    let rects = cells[0]
                        .as_ref()
                        .unwrap()
                        .as_arrow_ref()
                        .as_any()
                        .downcast_ref::<UnionArray>()
                        .unwrap();
                    assert_eq!(NUM_INSTANCES as usize, rects.len());
                });
            });
        }
    }
}

fn latest_at_missing(c: &mut Criterion) {
    for &packed in packed() {
        let mut group = c.benchmark_group(format!(
            "datastore/num_rows={NUM_ROWS}/num_instances={NUM_INSTANCES}/packed={packed}/latest_at_missing"
        ));
        group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

        let table = build_table(NUM_INSTANCES as usize, packed);

        // Default config
        let store = insert_table(Default::default(), InstanceKey::name(), &table);
        group.bench_function("primary/default", |b| {
            b.iter(|| {
                let results =
                    latest_data_at(&store, "non_existing_component".into(), &[Rect2D::name()]);
                assert!(results[0].is_none());
            });
        });
        group.bench_function("secondaries/default", |b| {
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

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            let store = insert_table(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                &table,
            );
            group.bench_function(format!("primary/bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let results =
                        latest_data_at(&store, "non_existing_component".into(), &[Rect2D::name()]);
                    assert!(results[0].is_none());
                });
            });
            group.bench_function(format!("secondaries/bucketsz={num_rows_per_bucket}"), |b| {
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
}

fn range(c: &mut Criterion) {
    for &packed in packed() {
        let mut group = c.benchmark_group(format!(
            "datastore/num_rows={NUM_ROWS}/num_instances={NUM_INSTANCES}/packed={packed}/range"
        ));
        group.throughput(criterion::Throughput::Elements(
            (NUM_INSTANCES * NUM_ROWS) as _,
        ));

        let table = build_table(NUM_INSTANCES as usize, packed);

        // Default config
        group.bench_function("default", |b| {
            b.iter(|| insert_table(Default::default(), InstanceKey::name(), &table));
        });

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            let store = insert_table(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                &table,
            );
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let rows = range_data(&store, [Rect2D::name()]);
                    for (cur_time, (time, cells)) in rows.enumerate() {
                        let time = time.unwrap();
                        assert_eq!(cur_time as i64, time.as_i64());

                        let rects = cells[0]
                            .as_ref()
                            .unwrap()
                            .as_arrow_ref()
                            .as_any()
                            .downcast_ref::<UnionArray>()
                            .unwrap();
                        assert_eq!(NUM_INSTANCES as usize, rects.len());
                    }
                });
            });
        }
    }
}

fn gc(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!(
        "datastore/num_rows={NUM_ROWS}/num_instances={NUM_INSTANCES}/gc"
    ));
    group.throughput(criterion::Throughput::Elements(
        (NUM_INSTANCES * NUM_ROWS) as _,
    ));

    let table = build_table(NUM_INSTANCES as usize, false);

    // Default config
    group.bench_function("default", |b| {
        let store = insert_table(Default::default(), InstanceKey::name(), &table);
        b.iter(|| {
            let mut store = store.clone();
            let (_, stats_diff) = store.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0));
            stats_diff
        });
    });

    // Emulate more or less bucket
    for &num_rows_per_bucket in num_rows_per_bucket() {
        group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
            let store = insert_table(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                &table,
            );
            b.iter(|| {
                let mut store = store.clone();
                let (_, stats_diff) =
                    store.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0));
                stats_diff
            });
        });
    }
}

// --- Helpers ---

fn build_table(n: usize, packed: bool) -> DataTable {
    let mut table = DataTable::from_rows(
        TableId::ZERO,
        (0..NUM_ROWS).map(move |frame_idx| {
            DataRow::from_cells2(
                RowId::random(),
                "rects",
                [build_frame_nr(frame_idx.into())],
                n as _,
                (build_some_instances(n), build_some_rects(n)),
            )
        }),
    );

    // Do a serialization roundtrip to pack everything in contiguous memory.
    if packed {
        let (schema, columns) = table.serialize().unwrap();
        table = DataTable::deserialize(TableId::ZERO, &schema, &columns).unwrap();
    }

    // NOTE: Using unsized cells will crash in debug mode, and benchmarks are run for 1 iteration,
    // in debug mode, by the standard test harness.
    if cfg!(debug_assertions) {
        table.compute_all_size_bytes();
    }

    table
}

fn insert_table(
    config: DataStoreConfig,
    cluster_key: ComponentName,
    table: &DataTable,
) -> DataStore {
    let mut store = DataStore::new(cluster_key, config);
    store.insert_table(table).unwrap();
    store
}

fn latest_data_at<const N: usize>(
    store: &DataStore,
    primary: ComponentName,
    secondaries: &[ComponentName; N],
) -> [Option<DataCell>; N] {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = LatestAtQuery::new(timeline_frame_nr, (NUM_ROWS / 2).into());
    let ent_path = EntityPath::from("rects");

    store
        .latest_at(&timeline_query, &ent_path, primary, secondaries)
        .map_or_else(|| [(); N].map(|_| None), |(_, cells)| cells)
}

fn range_data<const N: usize>(
    store: &DataStore,
    components: [ComponentName; N],
) -> impl Iterator<Item = (Option<TimeInt>, [Option<DataCell>; N])> + '_ {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline_frame_nr, TimeRange::new(0.into(), NUM_ROWS.into()));
    let ent_path = EntityPath::from("rects");

    store
        .range(&query, &ent_path, components)
        .map(move |(time, _, cells)| (time, cells))
}
