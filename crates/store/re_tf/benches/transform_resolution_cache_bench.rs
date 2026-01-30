#![expect(clippy::unwrap_used)] // acceptable in benchmarks

use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use itertools::Itertools as _;
use re_chunk_store::{Chunk, ChunkStoreEvent};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, TimePoint, Timeline, TimelineName};
use re_sdk_types::{RowId, archetypes};
use re_tf::{TransformFrameIdHash, TransformResolutionCache};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const NUM_TIMELINES: usize = 4;
const NUM_TIMEPOINTS: usize = 1000;
const NUM_TIMEPOINTS_PER_ENTITY: usize = 50;
const NUM_ENTITIES: usize = 100;

fn setup_store() -> (EntityDb, Vec<ChunkStoreEvent>) {
    let mut entity_db = EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));

    let timelines = (0..NUM_TIMELINES)
        .map(|i| Timeline::new(format!("timeline{i}"), re_log_types::TimeType::Sequence))
        .collect_vec();

    let mut events = Vec::new();
    for entity_idx in 0..NUM_ENTITIES {
        for batch in 0..(NUM_TIMEPOINTS / NUM_TIMEPOINTS_PER_ENTITY) {
            let chunk_base_time = batch * NUM_TIMEPOINTS_PER_ENTITY;

            let mut builder = Chunk::builder(EntityPath::from(format!("entity{entity_idx}")));
            for t in 0..NUM_TIMEPOINTS_PER_ENTITY {
                let mut timepoint = TimePoint::default();
                for timeline in &timelines {
                    #[expect(clippy::cast_possible_wrap)]
                    timepoint.insert(*timeline, (chunk_base_time + t) as i64);
                }
                builder = builder.with_archetype(
                    RowId::new(),
                    timepoint,
                    &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0])
                        .with_scale(2.0)
                        .with_quaternion(glam::Quat::from_xyzw(0.0, 2.0, 3.0, 1.0))
                        .with_mat3x3(glam::Mat3::IDENTITY),
                );
            }
            let chunk = builder.build().unwrap();

            events.extend(entity_db.add_chunk(&Arc::new(chunk)).unwrap().into_iter());
        }
    }
    (entity_db, events)
}

fn transform_resolution_cache_query(c: &mut Criterion) {
    let (entity_db, events) = setup_store();

    c.bench_function("build_from_entity_db", |b| {
        b.iter(|| {
            let mut cache = TransformResolutionCache::default();
            cache.process_store_events(events.iter());
            cache
        });
    });

    let query = re_chunk_store::LatestAtQuery::new(TimelineName::new("timeline2"), 123);
    let queried_frame = TransformFrameIdHash::from_entity_path(&EntityPath::from("entity2"));

    c.bench_function("query_uncached_frame", |b| {
        b.iter_batched(
            || {
                let mut cache = TransformResolutionCache::default();
                cache.process_store_events(events.iter());
                cache
            },
            |cold_cache| {
                let timeline_transforms = cold_cache.transforms_for_timeline(query.timeline());
                let frame_transforms = timeline_transforms.frame_transforms(queried_frame).unwrap();
                frame_transforms
                    .latest_at_transform(&entity_db, &query)
                    .unwrap()
            },
            criterion::BatchSize::PerIteration,
        );
    });

    let mut warm_cache = TransformResolutionCache::default();
    warm_cache.process_store_events(events.iter());
    let timeline_transforms = warm_cache.transforms_for_timeline(query.timeline());
    timeline_transforms
        .frame_transforms(queried_frame)
        .unwrap()
        .latest_at_transform(&entity_db, &query);

    c.bench_function("query_cached_frame", |b| {
        b.iter(|| {
            let timeline_transforms = warm_cache.transforms_for_timeline(query.timeline());
            let frame_transforms = timeline_transforms.frame_transforms(queried_frame).unwrap();
            frame_transforms
                .latest_at_transform(&entity_db, &query)
                .unwrap()
        });
    });

    // TODO(andreas): Additional benchmarks for iterative invalidation would be great!
}

criterion_group!(benches, transform_resolution_cache_query);
criterion_main!(benches);
