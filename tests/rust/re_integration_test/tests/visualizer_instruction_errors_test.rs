//! Test that per-visualizer instruction errors are correctly reported and displayed.

use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_test_context::VisualizerBlueprintContext as _;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_sdk_types::VisualizableArchetype as _;
use re_viewer::external::re_sdk_types::archetypes::{Scalars, SeriesLines, SeriesPoints};
use re_viewer::external::re_sdk_types::blueprint::datatypes::{
    ComponentSourceKind, VisualizerComponentMapping,
};
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn per_visualizer_instruction_errors() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
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

    let view_id = harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            TimeSeriesView::identifier(),
            RecommendedView::new_single_entity("scalars"),
        ))
    });

    // Add a visualizer with a warning (invalid mapping for optional component)
    let broken_color_mapping = VisualizerComponentMapping {
        target: SeriesPoints::descriptor_colors().component.as_str().into(),
        source_kind: ComponentSourceKind::SourceComponent,
        source_component: Some("non_existent_color".into()),
        selector: None,
    };
    {
        let broken_color_mapping = broken_color_mapping.clone();
        harness.setup_viewport_blueprint(move |viewer_context, _blueprint| {
            viewer_context.save_visualizers(
                &"scalars".into(),
                view_id,
                [SeriesPoints::default()
                    .visualizer()
                    .with_mappings([broken_color_mapping.into()])],
            );
        });
    }

    // Should show points with a warning.
    harness.snapshot_app("per_visualizer_instruction_errors_1_warnings_only");

    // Click to open the errors menu (should show both warnings and errors)
    harness.click_label("View warnings");
    harness.snapshot_app("per_visualizer_instruction_errors_1b_warnings_only_menu");

    // Now add a visualizer with an error and keep the warning.
    let broken_scalar_mapping = VisualizerComponentMapping {
        target: Scalars::descriptor_scalars().component.as_str().into(),
        source_kind: ComponentSourceKind::SourceComponent,
        source_component: Some("non_existent_scalars".into()),
        selector: None,
    };
    {
        let broken_scalar_mapping = broken_scalar_mapping.clone();
        harness.setup_viewport_blueprint(move |viewer_context, _blueprint| {
            viewer_context.save_visualizers(
                &"scalars".into(),
                view_id,
                [SeriesPoints::default()
                    .visualizer()
                    .with_mappings([broken_color_mapping.into(), broken_scalar_mapping.into()])],
            );
        });
    }

    // Menu should still be open.
    harness.snapshot_app("per_visualizer_instruction_errors_2_warnings_and_errors_menu");

    // Click on the entity path where the error occurs.
    harness.click_label("/scalars");
    harness.snapshot_app("per_visualizer_instruction_errors_2b_warnings_and_errors_dataresult");

    // Now test errors only - remove the warning visualizer.
    {
        harness.setup_viewport_blueprint(move |viewer_context, _blueprint| {
            viewer_context.save_visualizers(
                &"scalars".into(),
                view_id,
                [SeriesLines::default()
                    .visualizer()
                    .with_mappings([broken_scalar_mapping.into()])],
            );
        });
    }

    // Menu got closed because we clicked on an entity path.
    harness.snapshot_app("per_visualizer_instruction_errors_3_errors_only");

    // Open it up again.
    harness.click_label("View errors");
    harness.snapshot_app("per_visualizer_instruction_errors_3b_errors_only_menu");
}
