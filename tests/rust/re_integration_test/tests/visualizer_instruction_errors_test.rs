//! Test that per-visualizer instruction errors are correctly reported and displayed.

use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_sdk_types::VisualizableArchetype as _;
use re_viewer::external::re_sdk_types::archetypes::{Scalars, SeriesLines, SeriesPoints};
use re_viewer::external::re_sdk_types::blueprint::datatypes::{
    ComponentSourceKind, VisualizerComponentMapping,
};
use re_viewer::external::re_viewer_context::{
    BlueprintContext as _, RecommendedView, ViewClass as _,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn per_visualizer_instruction_errors() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        edit_app_options: Some(Box::new(|app_options| {
            app_options.experimental.component_mapping = true;
        })),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = Timeline::new_sequence("t");
    for t in 0..10 {
        harness.log_entity("scalars", |builder| {
            builder.with_archetype_auto_row([(timeline, t)], &Scalars::new([t as f64]))
        });
    }

    harness.setup_viewport_blueprint(|viewer_context, blueprint| {
        let view = ViewBlueprint::new(
            TimeSeriesView::identifier(),
            RecommendedView::new_single_entity("scalars"),
        );

        viewer_context.save_visualizers(
            &"scalars".into(),
            view.id,
            [
                // This one should work.
                SeriesPoints::default().visualizer(),
                // This one should error since one can't use defaults for the required component.
                SeriesLines::default()
                    .visualizer()
                    .with_mappings([VisualizerComponentMapping {
                        target: Scalars::descriptor_scalars().component.as_str().into(),
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    }
                    .into()]),
            ],
        );

        blueprint.add_view_at_root(view);
    });

    // Should show only points and an error.
    harness.snapshot_app("per_visualizer_instruction_errors_0");

    // Click the error menu button to open the popup (which should be there!)
    harness.click_label("View errors");
    harness.snapshot_app("per_visualizer_instruction_errors_1");

    // There should be an error we can inspect now.
    harness.click_label("/scalars");
    harness.snapshot_app("per_visualizer_instruction_errors_2");
}
