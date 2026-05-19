//! Test that view component defaults can be written and read back from the blueprint store.

use re_integration_test::HarnessExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_chunk::LatestAtQuery;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::{
    BlueprintContext as _, ViewClass as _, blueprint_timeline,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

/// Adds a `StrokeWidth` component default to a time series view and verifies
/// it can be read back from the blueprint store on the blueprint timeline.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_defaults_stroke_width() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 1000.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = re_sdk::Timeline::new_sequence("frame");

    // Log scalar data so the time series view has something to visualize.
    for i in 0..50 {
        harness.log_entity("plot", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 10.0).sin()),
            )
        });
    }

    // Create a time series view and save a StrokeWidth default on its defaults path.
    let (defaults_path, component_id) =
        harness.setup_viewport_blueprint(|viewer_context, blueprint| {
            let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
            let defaults_path = view.defaults_path.clone();

            let descriptor = re_sdk_types::archetypes::SeriesLines::descriptor_widths();
            let component_id = descriptor.component;
            let stroke_width = re_sdk_types::components::StrokeWidth::from(5.0f32);
            viewer_context.save_blueprint_component(
                defaults_path.clone(),
                &descriptor,
                &stroke_width,
            );

            blueprint.add_view_at_root(view);
            (defaults_path, component_id)
        });

    // Expand the blueprint tree and select the view to show defaults in the selection panel.
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");
    harness.run();

    // Select the view to show the selection panel with component defaults.
    harness.blueprint_tree().click_label("/");
    harness.run();

    harness.snapshot_app("view_defaults");

    // Verify the default is queryable from the blueprint store on the blueprint timeline.
    let stroke_width = harness.run_with_viewer_context(move |viewer_context| {
        let blueprint_db = viewer_context.store_context.blueprint;
        let query = LatestAtQuery::latest(blueprint_timeline());
        let results = blueprint_db.latest_at(&query, &defaults_path, [component_id]);
        results.component_mono::<re_sdk_types::components::StrokeWidth>(component_id)
    });

    let stroke_width =
        stroke_width.expect("StrokeWidth default should be queryable from blueprint store");
    assert_eq!(
        stroke_width.0.0, 5.0,
        "StrokeWidth should match the saved default value"
    );
}
