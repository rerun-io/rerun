// Tests for saving/loading blueprints to/from a file.
use std::path::Path;

use re_chunk::{RowId, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, ViewClass as _};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewBlueprint;

fn log_test_data_and_register_views(test_context: &mut TestContext, scalars_count: usize) {
    test_context.register_view_class::<re_view_dataframe::DataframeView>();
    test_context.register_view_class::<re_view_bar_chart::BarChartView>();

    let timeline_a = re_chunk::Timeline::new_sequence("timeline_a");
    test_context.log_entity("scalar", |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_a, 0)],
            &re_sdk_types::archetypes::Scalars::single(scalars_count as f32),
        )
    });

    let vector = (0..scalars_count).map(|i| i as f32).collect::<Vec<_>>();

    test_context.log_entity("vector", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::BarChart::new(vector),
        )
    });
}

fn setup_viewport(test_context: &mut TestContext) {
    let view_1 =
        ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    let view_2 =
        ViewBlueprint::new_with_root_wildcard(re_view_dataframe::DataframeView::identifier());

    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        // Set the color override for the bar chart view.
        ctx.save_visualizers(
            &re_chunk::EntityPath::from("vector"),
            view_1.id,
            [
                &re_sdk_types::archetypes::BarChart::default().with_color([255, 144, 1]), // #FF9001
            ],
        );

        // Set the timeline for the dataframe view.
        let query = re_view_dataframe::Query::from_blueprint(ctx, view_2.id);
        query.save_timeline_name(ctx, &re_chunk::TimelineName::from("timeline_a"));

        blueprint.add_views([view_1, view_2].into_iter(), None, None);
    });
}

fn save_blueprint_to_file(test_context: &TestContext, path: &Path) {
    test_context
        .save_blueprint_to_file(path)
        .expect("Failed to save blueprint to file.");
}

fn load_blueprint_from_file(test_context: &mut TestContext, path: &Path) {
    let file = std::fs::File::open(path).expect("Failed to open blueprint file.");
    let file = std::io::BufReader::new(file);
    let rbl_store =
        re_entity_db::StoreBundle::from_rrd(file).expect("Failed to load blueprint store");
    {
        let mut lock = test_context.store_hub.lock();
        let app_id = lock.active_app().expect("Missing active app").clone();
        lock.load_blueprint_store(rbl_store, &app_id)
            .expect("Failed to load blueprint store");
    }

    // Trigger recalculation of visualizable entities and blueprint overrides.
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});
}

fn take_snapshot(
    test_context: &TestContext,
    snapshot_name: &str,
    snapshot_results: &mut egui_kittest::SnapshotResults,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                let viewport_blueprint = re_viewport_blueprint::ViewportBlueprint::from_db(
                    ctx.blueprint_db(),
                    &test_context.blueprint_query,
                );
                let viewport_ui = ViewportUi::new(viewport_blueprint);
                viewport_ui.viewport_ui(ui, ctx, &mut test_context.view_states.lock());
            });

            test_context.handle_system_commands(ui.ctx());
        });
    harness.run();
    harness.snapshot(snapshot_name);
    snapshot_results.extend_harness(&mut harness);
}

#[test]
fn test_blueprint_change_and_restore() {
    let mut test_context = TestContext::new();
    log_test_data_and_register_views(&mut test_context, 16);
    let rbl_file = tempfile::NamedTempFile::new().unwrap();
    let rbl_path = rbl_file.path();

    setup_viewport(&mut test_context);
    save_blueprint_to_file(&test_context, rbl_path);

    // Remove the first view and add 3 new ones.
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let first_view_id = *blueprint.view_ids().next().unwrap();
        blueprint.remove_contents(re_viewer_context::Contents::View(first_view_id));
        blueprint.add_views([
            ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier()),
            ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier()),
            ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier()),
        ].into_iter(), None, None);
    });

    load_blueprint_from_file(&mut test_context, rbl_path);
    let mut snapshot_results = egui_kittest::SnapshotResults::new();
    take_snapshot(
        &test_context,
        "blueprint_change_and_restore",
        &mut snapshot_results,
    );
}

#[test]
fn test_blueprint_load_into_new_context() {
    let mut snapshot_results = egui_kittest::SnapshotResults::new();
    let mut test_context = TestContext::new();
    log_test_data_and_register_views(&mut test_context, 10);

    let rbl_file = tempfile::NamedTempFile::new().unwrap();
    let rbl_path = rbl_file.path();

    setup_viewport(&mut test_context);
    save_blueprint_to_file(&test_context, rbl_path);
    take_snapshot(
        &test_context,
        "blueprint_load_into_new_context_1",
        &mut snapshot_results,
    );

    let mut test_context_2 = TestContext::new();
    log_test_data_and_register_views(&mut test_context_2, 20);

    load_blueprint_from_file(&mut test_context_2, rbl_path);
    take_snapshot(
        &test_context_2,
        "blueprint_load_into_new_context_2",
        &mut snapshot_results,
    );
}
