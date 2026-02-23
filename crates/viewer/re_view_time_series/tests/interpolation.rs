use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::VisualizableArchetype as _;
use re_sdk_types::archetypes::{SeriesLines, SeriesPoints};
use re_sdk_types::components::{Color, InterpolationMode, MarkerShape};
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_context::{TestContext, VisualizerBlueprintContext as _};
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
fn test_interpolation_modes() {
    let mut snapshot_results = SnapshotResults::new();
    for mode in [
        InterpolationMode::Linear,
        InterpolationMode::StepAfter,
        InterpolationMode::StepBefore,
        InterpolationMode::StepMid,
    ] {
        let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

        let timeline = Timeline::log_tick();

        for step in 0..32 {
            let timepoint = TimePoint::from([(timeline, step)]);
            test_context.log_entity("plots/line", |builder| {
                builder.with_archetype(
                    RowId::new(),
                    timepoint,
                    &re_sdk_types::archetypes::Scalars::single((step as f64 / 5.0).sin()),
                )
            });
        }

        test_context.send_time_commands(
            test_context.active_store_id(),
            [TimeControlCommand::SetActiveTimeline(*timeline.name())],
        );

        let view_id = setup_blueprint(&mut test_context, mode);
        snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
            view_id,
            &format!("interpolation_mode_{mode:?}"),
            egui::vec2(300.0, 300.0),
            None,
        ));
    }
}

fn setup_blueprint(test_context: &mut TestContext, mode: InterpolationMode) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        ctx.save_visualizers(
            &EntityPath::from("plots/line"),
            view.id,
            [
                SeriesLines::new()
                    .with_interpolation_mode(mode)
                    .with_colors([Color::from_rgb(0, 200, 255)])
                    .with_names([format!("{mode:?}")])
                    .visualizer(),
                SeriesPoints::new()
                    .with_colors([Color::from_rgb(255, 100, 0)])
                    .with_markers([MarkerShape::Circle])
                    .with_marker_sizes([2.0])
                    .with_names([format!("{mode:?}")])
                    .visualizer(),
            ],
        );

        blueprint.add_view_at_root(view)
    })
}
