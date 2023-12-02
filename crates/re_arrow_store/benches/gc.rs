#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use itertools::Itertools;
use re_arrow_store::{
    DataStore, DataStoreConfig, GarbageCollectionOptions, GarbageCollectionTarget,
};
use re_log_types::{
    build_frame_nr, build_log_time, DataRow, DataTable, EntityPath, RowId, TableId, Time, TimePoint,
};
use re_types::components::InstanceKey;
use re_types_core::{AsComponents, ComponentBatch, ComponentName, Loggable as _};

criterion_group!(benches, plotting_dashboard, timeless_logs);
criterion_main!(benches);

// ---

#[cfg(not(debug_assertions))]
mod constants {
    pub const NUM_ENTITY_PATHS: usize = 20;
    pub const NUM_ROWS_PER_ENTITY_PATH: usize = 10_000;
}

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const NUM_ENTITY_PATHS: usize = 1;
    pub const NUM_ROWS_PER_ENTITY_PATH: usize = 1;
}

use constants::{NUM_ENTITY_PATHS, NUM_ROWS_PER_ENTITY_PATH};

fn num_rows_per_bucket() -> &'static [u64] {
    #[cfg(feature = "core_benchmarks_only")]
    {
        &[]
    }
    #[cfg(not(feature = "core_benchmarks_only"))]
    {
        &[256, 512, 1024, 2048]
    }
}

// --- Benchmarks ---

fn plotting_dashboard(c: &mut Criterion) {
    const DROP_AT_LEAST: f64 = 0.3;

    let mut group = c.benchmark_group(format!(
        "datastore/num_entities={NUM_ENTITY_PATHS}/num_rows_per_entity={NUM_ROWS_PER_ENTITY_PATH}/plotting_dashboard/drop_at_least={DROP_AT_LEAST}"
    ));
    group.throughput(criterion::Throughput::Elements(
        ((NUM_ENTITY_PATHS * NUM_ROWS_PER_ENTITY_PATH) as f64 * DROP_AT_LEAST) as _,
    ));
    group.sample_size(10);

    let gc_settings = GarbageCollectionOptions {
        target: GarbageCollectionTarget::DropAtLeastFraction(DROP_AT_LEAST),
        gc_timeless: true,
        protect_latest: 1,
        purge_empty_tables: false,
        dont_protect: Default::default(),
    };

    let mut timegen = |i| {
        [
            build_log_time(Time::from_seconds_since_epoch(i as _)),
            build_frame_nr((i as i64).into()),
        ]
        .into()
    };

    let mut datagen = |i| {
        Box::new(re_types::archetypes::TimeSeriesScalar::new(i as f64)) as Box<dyn AsComponents>
    };

    // Default config
    group.bench_function("default", |b| {
        let store = build_store(
            Default::default(),
            InstanceKey::name(),
            false,
            &mut timegen,
            &mut datagen,
        );
        b.iter_batched(
            || store.clone(),
            |mut store| {
                let (_, stats_diff) = store.gc(&gc_settings);
                stats_diff
            },
            BatchSize::LargeInput,
        );
    });

    // Emulate more or less bucket
    for &num_rows_per_bucket in num_rows_per_bucket() {
        group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
            let store = build_store(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                false,
                &mut timegen,
                &mut datagen,
            );
            b.iter_batched(
                || store.clone(),
                |mut store| {
                    let (_, stats_diff) = store.gc(&gc_settings);
                    stats_diff
                },
                BatchSize::LargeInput,
            );
        });
    }
}

fn timeless_logs(c: &mut Criterion) {
    const DROP_AT_LEAST: f64 = 0.3;

    let mut group = c.benchmark_group(format!(
        "datastore/num_entities={NUM_ENTITY_PATHS}/num_rows_per_entity={NUM_ROWS_PER_ENTITY_PATH}/timeless_logs/drop_at_least={DROP_AT_LEAST}"
    ));
    group.throughput(criterion::Throughput::Elements(
        ((NUM_ENTITY_PATHS * NUM_ROWS_PER_ENTITY_PATH) as f64 * DROP_AT_LEAST) as _,
    ));
    group.sample_size(10);

    let gc_settings = GarbageCollectionOptions {
        target: GarbageCollectionTarget::DropAtLeastFraction(DROP_AT_LEAST),
        gc_timeless: true,
        protect_latest: 1,
        purge_empty_tables: false,
        dont_protect: Default::default(),
    };

    let mut timegen = |_| TimePoint::timeless();

    let mut datagen = |i: usize| {
        Box::new(re_types::archetypes::TextLog::new(i.to_string())) as Box<dyn AsComponents>
    };

    // Default config
    group.bench_function("default", |b| {
        let store = build_store(
            Default::default(),
            InstanceKey::name(),
            false,
            &mut timegen,
            &mut datagen,
        );
        b.iter_batched(
            || store.clone(),
            |mut store| {
                let (_, stats_diff) = store.gc(&gc_settings);
                stats_diff
            },
            BatchSize::LargeInput,
        );
    });

    // Emulate more or less bucket
    for &num_rows_per_bucket in num_rows_per_bucket() {
        group.bench_function(format!("bucketsz={num_rows_per_bucket}"), |b| {
            let store = build_store(
                DataStoreConfig {
                    indexed_bucket_num_rows: num_rows_per_bucket,
                    ..Default::default()
                },
                InstanceKey::name(),
                false,
                &mut timegen,
                &mut datagen,
            );
            b.iter_batched(
                || store.clone(),
                |mut store| {
                    let (_, stats_diff) = store.gc(&gc_settings);
                    stats_diff
                },
                BatchSize::LargeInput,
            );
        });
    }
}

// --- Helpers ---

fn build_store<FT, FD>(
    config: DataStoreConfig,
    cluster_key: ComponentName,
    packed: bool,
    timegen: &mut FT,
    datagen: &mut FD,
) -> DataStore
where
    FT: FnMut(usize) -> TimePoint,
    FD: FnMut(usize) -> Box<dyn AsComponents>,
{
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        cluster_key,
        config,
    );

    let tables = (0..NUM_ENTITY_PATHS)
        .map(|i| build_table(format!("entity_path_{i}").into(), packed, timegen, datagen))
        .collect_vec();
    let mut rows_per_table = tables.iter().map(|table| table.to_rows()).collect_vec();

    // NOTE: interleave insertions between entities to more closely match real world scenarios.
    for _ in 0..NUM_ROWS_PER_ENTITY_PATH {
        #[allow(clippy::needless_range_loop)] // readability
        for i in 0..NUM_ENTITY_PATHS {
            let row = rows_per_table[i].next().unwrap();
            store.insert_row(&row.unwrap()).unwrap();
        }
    }

    store
}

fn build_table<FT, FD>(
    entity_path: EntityPath,
    packed: bool,
    timegen: &mut FT,
    datagen: &mut FD,
) -> DataTable
where
    FT: FnMut(usize) -> TimePoint,
    FD: FnMut(usize) -> Box<dyn AsComponents>,
{
    let mut table = DataTable::from_rows(
        TableId::ZERO,
        (0..NUM_ROWS_PER_ENTITY_PATH).map(move |i| {
            DataRow::from_component_batches(
                RowId::random(),
                // NOTE: insert in multiple timelines to more closely match real world scenarios.
                timegen(i),
                entity_path.clone(),
                datagen(i)
                    .as_component_batches()
                    .iter()
                    .map(|batch| batch as &dyn ComponentBatch),
            )
            .unwrap()
        }),
    );

    // Do a serialization roundtrip to pack everything in contiguous memory.
    if packed {
        let (schema, columns) = table.serialize().unwrap();
        table = DataTable::deserialize(TableId::ZERO, &schema, &columns).unwrap();
    }

    table.compute_all_size_bytes();

    table
}
