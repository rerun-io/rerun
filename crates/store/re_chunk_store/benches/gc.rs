#![expect(clippy::unwrap_used)] // acceptable in benchmarks

use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};

use re_chunk::{TimeInt, TimePoint};
use re_chunk_store::{Chunk, ChunkStore, ChunkStoreConfig, GarbageCollectionOptions};
use re_log_types::{StoreId, Timeline, TimelineName};
use re_sdk_types::{RowId, archetypes};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const NUM_CHUNKS: i64 = 10_000;
const NUM_ROWS_PER_CHUNK: i64 = 1_000;

fn setup_store() -> ChunkStore {
    let store_id = StoreId::random(re_log_types::StoreKind::Recording, "benchmarks");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::COMPACTION_DISABLED);

    for i in 0..NUM_CHUNKS {
        let timepoint = (i * NUM_ROWS_PER_CHUNK..i * NUM_ROWS_PER_CHUNK + NUM_ROWS_PER_CHUNK)
            .map(|t| (Timeline::log_tick(), t))
            .collect::<TimePoint>();
        let p = i as f64;
        let chunk = Chunk::builder("my_entity")
            .with_archetype(
                RowId::new(),
                timepoint,
                &archetypes::Points3D::new([[p, p, p]]),
            )
            .build()
            .unwrap();
        store.insert_chunk(&Arc::new(chunk)).unwrap();
    }

    store
}

fn gc(c: &mut Criterion) {
    let store = setup_store();

    c.bench_function("gc_everything/ordered_by_min_row_id", |b| {
        b.iter_batched(
            || store.clone(),
            |mut store| {
                assert_eq!(NUM_CHUNKS as usize, store.num_physical_chunks());
                store.gc(&GarbageCollectionOptions::gc_everything());
                assert_eq!(0, store.num_physical_chunks());
            },
            criterion::BatchSize::PerIteration,
        );
    });

    c.bench_function("gc_everything/ordered_by_distance_from_cursor", |b| {
        b.iter_batched(
            || store.clone(),
            |mut store| {
                assert_eq!(NUM_CHUNKS as usize, store.num_physical_chunks());
                store.gc(&GarbageCollectionOptions {
                    furthest_from: Some((
                        TimelineName::log_tick(),
                        TimeInt::new_temporal(NUM_CHUNKS / 2),
                    )),
                    ..GarbageCollectionOptions::gc_everything()
                });
                assert_eq!(0, store.num_physical_chunks());
            },
            criterion::BatchSize::PerIteration,
        );
    });
}

criterion_group!(benches, gc);
criterion_main!(benches);
