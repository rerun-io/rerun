#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::array::{Array as _, StructArray, UnionArray};
use criterion::{criterion_group, criterion_main, Criterion};

use re_data_store::{
    DataStore, DataStoreConfig, GarbageCollectionOptions, GarbageCollectionTarget, LatestAtQuery,
    RangeQuery, TimeInt, TimeRange,
};
use re_log_types::{
    build_frame_nr, DataCell, DataRow, DataTable, EntityPath, RowId, TableId, TimePoint, TimeType,
    Timeline,
};
use re_types::datagen::build_some_instances;
use re_types::{
    components::InstanceKey,
    testing::{build_some_large_structs, LargeStruct},
};
use re_types_core::{ComponentName, Loggable as _};

criterion_group!(
    benches,
    insert,
    insert_same_time_point,
    latest_at,
    latest_at_missing,
    range,
    gc
);
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
    if std::env::var("CI").is_ok() {
        &[false]
    } else {
        &[false, true]
    }
}

fn num_rows_per_bucket() -> &'static [u64] {
    if std::env::var("CI").is_ok() {
        &[]
    } else {
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

        let rows = build_rows_with_packed(packed);

        // Default config
        group.bench_function("default", |b| {
            b.iter(|| insert_rows(Default::default(), &rows));
        });

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    insert_rows(
                        DataStoreConfig {
                            indexed_bucket_num_rows: num_rows_per_bucket,
                            ..Default::default()
                        },
                        &rows,
                    )
                });
            });
        }
    }
}

fn insert_same_time_point(c: &mut Criterion) {
    // Benchmark a corner-case where all rows have the same time point, and arrive out-of-order.
    // See https://github.com/rerun-io/rerun/issues/4415

    // `cargo test` also runs the benchmark setup code, so make sure they run quickly:
    #[cfg(debug_assertions)]
    let num_rows_list = [100];
    #[cfg(not(debug_assertions))]
    let num_rows_list = [1_000, 10_000, 50_000];

    for num_rows in num_rows_list {
        for shuffled in [false, true] {
            let num_instances = 1;
            let packed = false;
            let mut group = c.benchmark_group(format!(
                "datastore/num_rows={num_rows}/num_instances={num_instances}/insert_same_time_point/shuffled={shuffled}"
            ));
            group.sample_size(10); // it is so slow
            group.throughput(criterion::Throughput::Elements(num_rows * num_instances));

            let rows = build_rows_ex(num_rows as _, num_instances as _, shuffled, packed, |_| {
                TimePoint::from([build_frame_nr(TimeInt::from(0))])
            });

            // Default config
            group.bench_function("insert", |b| {
                b.iter(|| {
                    let mut store = DataStore::new(
                        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
                        InstanceKey::name(),
                        DataStoreConfig::default(),
                    );

                    let num_ingested_rows_per_sort = 10;

                    for chunk in rows.chunks(num_ingested_rows_per_sort) {
                        for row in chunk {
                            store.insert_row(row).unwrap();
                        }

                        // This mimics the sorting required by a query.
                        // This benchmark emulating a streaming ingestion
                        // done concurrently with a bunch of queries.
                        // We ingest a bunch of rows, then query/sort them, then repeat.
                        store.sort_indices_if_needed();
                    }
                    store
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

        let rows = build_rows_with_packed(packed);

        // Default config
        group.bench_function("default", |b| {
            let store = insert_rows(Default::default(), &rows);
            b.iter(|| {
                let cells = latest_data_at(&store, LargeStruct::name(), &[LargeStruct::name()]);
                let large_structs = cells[0]
                    .as_ref()
                    .unwrap()
                    .as_arrow_ref()
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .unwrap();
                assert_eq!(NUM_INSTANCES as usize, large_structs.len());
            });
        });

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            let store = insert_rows(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                &rows,
            );
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let cells = latest_data_at(&store, LargeStruct::name(), &[LargeStruct::name()]);
                    let large_structs = cells[0]
                        .as_ref()
                        .unwrap()
                        .as_arrow_ref()
                        .as_any()
                        .downcast_ref::<StructArray>()
                        .unwrap();
                    assert_eq!(NUM_INSTANCES as usize, large_structs.len());
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

        let rows = build_rows_with_packed(packed);

        // Default config
        let store = insert_rows(Default::default(), &rows);
        group.bench_function("primary/default", |b| {
            b.iter(|| {
                let results = latest_data_at(
                    &store,
                    "non_existing_component".into(),
                    &[LargeStruct::name()],
                );
                assert!(results[0].is_none());
            });
        });
        group.bench_function("secondaries/default", |b| {
            b.iter(|| {
                let results = latest_data_at(
                    &store,
                    LargeStruct::name(),
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
            let store = insert_rows(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                &rows,
            );
            group.bench_function(format!("primary/bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let results = latest_data_at(
                        &store,
                        "non_existing_component".into(),
                        &[LargeStruct::name()],
                    );
                    assert!(results[0].is_none());
                });
            });
            group.bench_function(format!("secondaries/bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let results = latest_data_at(
                        &store,
                        LargeStruct::name(),
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

        let rows = build_rows_with_packed(packed);

        // Default config
        group.bench_function("default", |b| {
            b.iter(|| insert_rows(Default::default(), &rows));
        });

        // Emulate more or less buckets
        for &num_rows_per_bucket in num_rows_per_bucket() {
            let store = insert_rows(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                &rows,
            );
            group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
                b.iter(|| {
                    let rows = range_data(&store, [LargeStruct::name()]);
                    for (cur_time, (time, cells)) in rows.enumerate() {
                        let time = time.unwrap();
                        assert_eq!(cur_time as i64, time.as_i64());

                        let large_structs = cells[0]
                            .as_ref()
                            .unwrap()
                            .as_arrow_ref()
                            .as_any()
                            .downcast_ref::<UnionArray>()
                            .unwrap();
                        assert_eq!(NUM_INSTANCES as usize, large_structs.len());
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

    let rows = build_rows_with_packed(false);

    // Default config
    group.bench_function("default", |b| {
        let store = insert_rows(Default::default(), &rows);
        b.iter(|| {
            let mut store = store.clone();
            let (_, stats_diff) = store.gc(&GarbageCollectionOptions {
                target: GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0),
                gc_timeless: false,
                protect_latest: 0,
                purge_empty_tables: false,
                dont_protect: Default::default(),
                enable_batching: false,
                time_budget: std::time::Duration::MAX,
            });
            stats_diff
        });
    });

    // Emulate more or less bucket
    for &num_rows_per_bucket in num_rows_per_bucket() {
        group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
            let store = insert_rows(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                &rows,
            );
            b.iter(|| {
                let mut store = store.clone();
                let (_, stats_diff) = store.gc(&GarbageCollectionOptions {
                    target: GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0),
                    gc_timeless: false,
                    protect_latest: 0,
                    purge_empty_tables: false,
                    dont_protect: Default::default(),
                    enable_batching: false,
                    time_budget: std::time::Duration::MAX,
                });
                stats_diff
            });
        });
    }
}

// --- Helpers ---

fn build_rows_with_packed(packed: bool) -> Vec<DataRow> {
    build_rows_ex(
        NUM_ROWS as _,
        NUM_INSTANCES as _,
        false,
        packed,
        |row_idx| TimePoint::from([build_frame_nr((row_idx as i64).into())]),
    )
}

fn build_rows_ex(
    num_rows: usize,
    num_instances: usize,
    shuffled: bool,
    packed: bool,
    time_point: impl Fn(usize) -> TimePoint,
) -> Vec<DataRow> {
    let rows = (0..num_rows).map(move |frame_idx| {
        DataRow::from_cells2(
            RowId::new(),
            "large_structs",
            time_point(frame_idx),
            num_instances as _,
            (
                build_some_instances(num_instances),
                build_some_large_structs(num_instances),
            ),
        )
        .unwrap()
    });

    let mut table = if shuffled {
        use rand::seq::SliceRandom as _;
        let mut rows: Vec<DataRow> = rows.collect();
        rows.shuffle(&mut rand::thread_rng());
        DataTable::from_rows(TableId::ZERO, rows)
    } else {
        DataTable::from_rows(TableId::ZERO, rows)
    };

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

    table.to_rows().map(|r| r.unwrap()).collect()
}

fn insert_rows(config: DataStoreConfig, rows: &[DataRow]) -> DataStore {
    let cluster_key = InstanceKey::name();
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        cluster_key,
        config,
    );
    for row in rows {
        store.insert_row(row).unwrap();
    }
    store
}

fn latest_data_at<const N: usize>(
    store: &DataStore,
    primary: ComponentName,
    secondaries: &[ComponentName; N],
) -> [Option<DataCell>; N] {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_query = LatestAtQuery::new(timeline_frame_nr, (NUM_ROWS / 2).into());
    let ent_path = EntityPath::from("large_structs");

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
    let ent_path = EntityPath::from("large_structs");

    store
        .range(&query, &ent_path, components)
        .map(move |(time, _, cells)| (time, cells))
}
