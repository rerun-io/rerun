use std::f64::consts::TAU;

use re_integration_test::HarnessExt as _;
use re_sdk::log::RowId;
use re_viewer::external::re_types;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(true);

    let timeline = re_sdk::Timeline::new_sequence("timeline_a");
    for i in 0..100 {
        harness.log_entity("sin_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_types::archetypes::Scalars::single((i as f64 / 100.0 * TAU).sin()),
            )
        });
        harness.log_entity("line_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_types::archetypes::Scalars::single(i as f64 / 100.0),
            )
        });
    }

    // Set up a multi-view blueprint
    harness.clear_current_blueprint();

    let mut view = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView {
            origin: "/".into(),
            query_filter: re_sdk::external::re_log_types::EntityPathFilter::default(),
        },
    );
    view.display_name = Some("Plot".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_view_at_root(view)
    });

    harness
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drop_stream_to_view() {
    let mut harness = make_harness();
    harness
        .blueprint_tree()
        .click_label("Viewport (Grid container)");

    let drop_point = harness.get_panel_position("Plot").center();

    // Drag "sin_curve" to the plot
    harness.streams_tree().drag_label("sin_curve");
    harness.hover_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_1");
    harness.drop_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_2");

    // Try again, should fail
    harness.streams_tree().drag_label("sin_curve");
    harness.hover_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_3");
    harness.drop_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_4");

    // Drag "line_curve" to the plot
    harness.streams_tree().drag_label("line_curve");
    harness.hover_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_5");
    harness.drop_at(drop_point);
    harness.snapshot_app("drop_stream_to_view_6");
}
