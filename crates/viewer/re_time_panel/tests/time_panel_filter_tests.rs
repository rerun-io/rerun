#![cfg(feature = "testing")]

use re_chunk_store::external::re_chunk::ChunkBuilder;
use re_chunk_store::{LatestAtQuery, RowId};
use re_log_types::TimePoint;
use re_sdk_types::archetypes::Points3D;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_time_panel::{StreamsTreeData, TimePanel, TimePanelSource};
use re_ui::filter_widget::FilterState;
use re_viewer_context::blueprint_timeline;
use re_viewport_blueprint::ViewportBlueprint;

fn filter_queries() -> impl Iterator<Item = Option<&'static str>> {
    [
        None,
        Some("t"),
        Some("void"),
        Some("path"),
        Some("ath t"),
        Some("ath left"),
        Some("to/the"),
        Some("/to/the"),
        Some("to/the/"),
        Some("/to/the/"),
        Some("to/the oid"),
        Some("/path/to /rig"),
    ]
    .into_iter()
}

#[test]
pub fn test_various_filter_ui_snapshot() {
    TimePanel::ensure_registered_subscribers();

    let mut snapshot_results = SnapshotResults::new();
    for filter_query in filter_queries() {
        let test_context = prepare_test_context();

        let mut time_panel = TimePanel::default();
        if let Some(query) = filter_query {
            time_panel.activate_filter(query);
        }

        run_time_panel_and_save_snapshot(
            &test_context,
            time_panel,
            &format!(
                "various_filters-{}",
                filter_query
                    .map(|s| s.replace(' ', ",").replace('/', "_"))
                    .unwrap_or_else(|| "none".to_owned())
            ),
            &mut snapshot_results,
        );
    }
}

#[test]
pub fn test_various_filter_insta_snapshot() {
    for filter_query in filter_queries() {
        let test_context = prepare_test_context();

        let streams_tree_data = test_context.run_once_in_egui_central_panel(|viewer_ctx, _| {
            let mut filter_state = FilterState::default();

            if let Some(filter_query) = filter_query {
                filter_state.activate(filter_query);
            }

            StreamsTreeData::from_source_and_filter(
                viewer_ctx,
                TimePanelSource::Recording,
                &filter_state.filter(),
            )
        });

        let snapshot_name = format!(
            "various_filters-{}",
            filter_query
                .map(|s| s.replace(' ', ",").replace('/', "_"))
                .unwrap_or_else(|| "none".to_owned())
        );

        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.bind(|| {
            insta::assert_yaml_snapshot!(snapshot_name, streams_tree_data);
        });
    }
}

fn prepare_test_context() -> TestContext {
    let mut test_context = TestContext::new();

    test_context.log_entity("/path/to/left", add_point_to_chunk_builder);
    test_context.log_entity("/path/to/right", add_point_to_chunk_builder);
    test_context.log_entity("/path/to/the/void", add_point_to_chunk_builder);
    test_context.log_entity("/path/onto/their/coils", add_point_to_chunk_builder);
    test_context.log_entity("/center/way", add_point_to_chunk_builder);

    // also populate some "intermediate" entities so we see components
    test_context.log_entity("/path", add_point_to_chunk_builder);
    test_context.log_entity("/path/to", add_point_to_chunk_builder);

    test_context
}

fn add_point_to_chunk_builder(builder: ChunkBuilder) -> ChunkBuilder {
    // log as static to minimize "noise" in the snapshot
    builder.with_archetype(
        RowId::new(),
        TimePoint::default(),
        &Points3D::new([[0.0, 0.0, 0.0]]),
    )
}

fn run_time_panel_and_save_snapshot(
    test_context: &TestContext,
    mut time_panel: TimePanel,
    snapshot_name: &str,
    snapshot_results: &mut SnapshotResults,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui([700.0, 700.0])
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                let blueprint = ViewportBlueprint::from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                );

                let mut time_commands = Vec::new();

                time_panel.show_expanded_with_header(
                    viewer_ctx,
                    viewer_ctx.time_ctrl,
                    &blueprint,
                    viewer_ctx.recording(),
                    ui,
                    &mut time_commands,
                );

                test_context.send_time_commands(viewer_ctx.store_id().clone(), time_commands);
            });

            test_context.handle_system_commands(ui.ctx());
        });

    harness.run();
    harness.snapshot(snapshot_name);

    snapshot_results.extend_harness(&mut harness);
}
