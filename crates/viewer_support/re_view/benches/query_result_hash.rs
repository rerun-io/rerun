use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use re_chunk_store::{LatestAtQuery, RangeQuery};
use re_log_types::{AbsoluteTimeRange, EntityPath, Timeline};
use re_sdk_types::{
    Archetype as _, archetypes::TextLog, blueprint::components::VisualizerInstructionId,
};
use re_test_context::TestContext;
use re_view::{latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data};
use re_viewer_context::{
    DataQueryResult, DataResult, QueryRange, ViewContext, ViewId, VisualizerComponentMappings,
    VisualizerInstruction,
};

/// Benchmarks for the `query_result_hash` method of both latest-at and range queries.
///
/// At some point it has been atrociously slow for large queries, so here we make sure it stays small.
/// (a user should reasonably expect it to be in the order of nanoseconds)
fn query_result_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_result_hash");
    for num_rows in [1, 100_000, 10_000_000] {
        let mut test_context = TestContext::new();
        let timeline = Timeline::log_tick();
        let log = TextLog::new("message")
            .with_level("INFO")
            .with_color([255, 128, 0]);
        // Bound builder memory while exercising ranges spanning many chunks.
        for start in (0..num_rows).step_by(10_000) {
            test_context.log_entity("logs", |builder| {
                (start..(start + 10_000).min(num_rows)).fold(builder, |builder, tick| {
                    builder.with_archetype_auto_row([(timeline, tick)], &log)
                })
            });
        }
        let data_result = DataResult {
            entity_path: "logs".into(),
            any_visualizers_available: true,
            visualizer_instructions: Vec::new(),
            tree_prefix_only: false,
            visible: true,
            interactive: true,
            override_base_path: "override".into(),
            query_range: QueryRange::LatestAt,
        };
        let instruction = VisualizerInstruction::new(
            VisualizerInstructionId::new_random(),
            "TextLog".into(),
            data_result.override_base_path(),
            VisualizerComponentMappings::default(),
        );

        test_context.run(&egui::Context::default(), |viewer_ctx| {
            let ctx = ViewContext {
                viewer_ctx,
                view_id: ViewId::invalid(),
                view_class_identifier: "TextLog".into(),
                space_origin: &EntityPath::root(),
                view_state: &(),
                query_result: &DataQueryResult::default(),
            };
            if num_rows == 1 {
                let latest_query = LatestAtQuery::latest(*timeline.name());
                let latest = latest_at_with_blueprint_resolved_data(
                    &ctx,
                    None,
                    &latest_query,
                    &data_result,
                    TextLog::all_component_identifiers(),
                    Some(&instruction),
                );
                group.bench_function("latest_at", |b| {
                    b.iter(|| black_box(&latest).query_result_hash());
                });
            }
            let range_query = RangeQuery::new(*timeline.name(), AbsoluteTimeRange::EVERYTHING)
                .keep_extra_timelines(true);
            let range = range_with_blueprint_resolved_data(
                &ctx,
                None,
                &range_query,
                &data_result,
                TextLog::all_component_identifiers(),
                &instruction,
            );

            // Query execution, including dependency fingerprint construction, is excluded.
            group.bench_function(BenchmarkId::new("range", num_rows), |b| {
                b.iter(|| black_box(&range).query_result_hash());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, query_result_hash);
criterion_main!(benches);
