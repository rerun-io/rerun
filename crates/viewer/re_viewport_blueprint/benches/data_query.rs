// Allow unwrap() in benchmarks
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use re_chunk::{Chunk, RowId};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathFilter, EntityPathSubs, StoreId, TimePoint, Timeline};
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::Position2D;
use re_types_core::ViewClassIdentifier;
use re_viewer_context::{
    Caches, PerVisualizer, PerVisualizerInViewClass, QueryRange, StoreContext, ViewClassRegistry,
    VisualizableEntities, VisualizableReason, blueprint_timeline,
};
use re_viewport_blueprint::ViewContents;

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const NUM_PARENTS: usize = 2;
    pub const NUM_CHILDREN_PER_PARENT: usize = 2;
    pub const NUM_GRANDCHILDREN_PER_CHILD: usize = 2;
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const NUM_PARENTS: usize = 80;
    pub const NUM_CHILDREN_PER_PARENT: usize = 18;
    pub const NUM_GRANDCHILDREN_PER_CHILD: usize = 6;
}

use self::constants::{NUM_CHILDREN_PER_PARENT, NUM_GRANDCHILDREN_PER_CHILD, NUM_PARENTS};

// ---

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion_group!(benches, query_tree_many_entities);
criterion_main!(benches);

// ---

fn query_tree_many_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_query_tree");

    let view_class_id = "TestClass".into();

    let num_entities = NUM_PARENTS * NUM_CHILDREN_PER_PARENT * NUM_GRANDCHILDREN_PER_CHILD;
    group.throughput(criterion::Throughput::Elements(num_entities as _));

    let (recording, visualizable_entities) = build_entity_tree(view_class_id);
    let indicated_entities = PerVisualizer(
        // Indicate everything.
        visualizable_entities
            .per_visualizer
            .iter()
            .map(|(v, e)| {
                (
                    *v,
                    re_viewer_context::IndicatedEntities(e.keys().cloned().collect()),
                )
            })
            .collect(),
    );

    let blueprint = EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Blueprint,
        "bench_app",
    ));

    let ctx = StoreContext {
        blueprint: &blueprint,
        default_blueprint: None,
        recording: &recording,
        caches: &Caches::new(recording.store_id().clone()),
        should_enable_heuristics: false,
    };

    let view_class_registry = ViewClassRegistry::default();
    let blueprint_query = LatestAtQuery::latest(blueprint_timeline());
    let active_timeline = Timeline::new_sequence("frame");
    let query_range = QueryRange::LatestAt;

    // Benchmark with simple include-all filter
    {
        let view_contents = ViewContents::new(
            re_viewer_context::ViewId::random(),
            view_class_id,
            EntityPathFilter::parse_forgiving("+ /**").resolve_forgiving(&EntityPathSubs::empty()),
        );

        group.bench_function("include_all", |b| {
            b.iter(|| {
                view_contents.build_data_result_tree(
                    &ctx,
                    Some(&active_timeline),
                    &view_class_registry,
                    &blueprint_query,
                    &query_range,
                    &visualizable_entities,
                    &indicated_entities,
                )
            });
        });
    }

    // Benchmark with complex filter rules
    {
        let filter_str = r"
            + /**
            - parent_0/child_0/**
            + parent_0/child_0/leaf_0
            - parent_42/**
            + parent_42/child_5
            - parent_55/child_10/**
        ";
        let view_contents = ViewContents::new(
            re_viewer_context::ViewId::random(),
            "3D".into(),
            EntityPathFilter::parse_forgiving(filter_str)
                .resolve_forgiving(&EntityPathSubs::empty()),
        );

        group.bench_function("complex_filter", |b| {
            b.iter(|| {
                view_contents.build_data_result_tree(
                    &ctx,
                    Some(&active_timeline),
                    &view_class_registry,
                    &blueprint_query,
                    &query_range,
                    &visualizable_entities,
                    &indicated_entities,
                )
            });
        });
    }

    group.finish();
}

// --- Helpers ---

fn build_entity_tree(
    view_class_id: ViewClassIdentifier,
) -> (EntityDb, PerVisualizerInViewClass<VisualizableEntities>) {
    use rand::{Rng as _, SeedableRng as _};
    // Use a fixed seed for deterministic, reproducible benchmarks
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);

    let mut recording = EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "bench_app",
    ));

    let timeline = Timeline::new_sequence("frame");
    let timepoint = TimePoint::from_iter([(timeline, 0)]);

    let mut all_entities = Vec::new();

    // Build a hierarchical entity tree
    for parent_idx in 0..NUM_PARENTS {
        let parent_path = format!("parent_{parent_idx}");
        all_entities.push(EntityPath::from(parent_path.as_str()));

        for child_idx in 0..NUM_CHILDREN_PER_PARENT {
            let child_path = format!("{parent_path}/child_{child_idx}");
            all_entities.push(EntityPath::from(child_path.as_str()));

            for grandchild_idx in 0..NUM_GRANDCHILDREN_PER_CHILD {
                let leaf_path = format!("{child_path}/leaf_{grandchild_idx}");
                all_entities.push(EntityPath::from(leaf_path.as_str()));

                // Randomly add an extra level of depth
                if rng.random_bool(0.2) {
                    let extra_path = format!("{leaf_path}/extra");
                    all_entities.push(EntityPath::from(extra_path.as_str()));
                }
            }
        }
    }

    // Add some data to the entities
    for entity_path in &all_entities {
        let row_id = RowId::new();
        let position = Position2D::new(rng.random_range(0.0..100.0), rng.random_range(0.0..100.0));

        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(row_id, timepoint.clone(), &Points2D::new(vec![position]))
            .build()
            .unwrap();

        recording.add_chunk(&Arc::new(chunk)).unwrap();
    }

    // Set up visualizable entities - make most entities visualizable
    let mut visualizable_entities =
        PerVisualizerInViewClass::<VisualizableEntities>::empty(view_class_id);
    let visualizable_set = all_entities
        .iter()
        .filter(|_| rng.random_bool(0.7)) // 70% of entities are visualizable
        .cloned()
        .map(|e| (e, VisualizableReason::Always))
        .collect();

    visualizable_entities
        .per_visualizer
        .insert("Points3D".into(), VisualizableEntities(visualizable_set));

    (recording, visualizable_entities)
}
