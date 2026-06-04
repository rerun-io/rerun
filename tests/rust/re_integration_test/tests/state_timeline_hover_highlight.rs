//! Cross-view time-range highlighting: hovering over a state phase in the state
//! timeline view should publish a `TimeRangeHighlight` that the time series view
//! (and any other view on the same timeline) picks up and paints as a background
//! band.

use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_sdk::log::RowId;
use re_view_state_timeline::StateTimelineView;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::{RecommendedView, TimeControlCommand, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_state_phase_hover_propagates_to_time_series() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(true);

    let timeline = Timeline::new_sequence("frame");
    let timeline_name = *timeline.name();

    // State phases on the "frame" timeline.
    let state_phases: [(i64, &str); 3] = [(0, "Idle"), (40, "Moving"), (80, "Done")];
    for (t, state) in &state_phases {
        harness.log_entity("state/robot", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, *t)],
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    // Scalars on the same timeline so the time series view has data to plot.
    for t in 0..120i64 {
        harness.log_entity("scalars/value", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, t)],
                &re_sdk_types::archetypes::Scalars::single((t as f64 / 20.0).sin()),
            )
        });
    }

    // Make the active timeline match the data we logged so both views query "frame".
    harness.run_with_viewer_context(move |viewer_context| {
        viewer_context.send_time_commands([TimeControlCommand::SetActiveTimeline(timeline_name)]);
    });
    harness.run();

    // Two named views, one above the other.
    harness.clear_current_blueprint();
    let mut state_view = ViewBlueprint::new(
        StateTimelineView::identifier(),
        RecommendedView::new_single_entity("state/robot"),
    );
    state_view.display_name = Some("State view".into());

    let mut ts_view = ViewBlueprint::new(
        TimeSeriesView::identifier(),
        RecommendedView::new_single_entity("scalars/value"),
    );
    ts_view.display_name = Some("Time series view".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views([state_view, ts_view].into_iter(), None, None);
    });
    harness.run();

    // Baseline: no hover, no highlight band on either view.
    harness.snapshot_app("state_timeline_hover_highlight_before");

    // Position over the first lane band, inside the middle phase ("Moving", 40..80).
    // The view auto-fits to the data plus a trailing overhang, so the middle phase
    // sits a bit left of center.
    let state_rect = harness.get_panel_position("State view");
    let hover_pos = egui::pos2(
        state_rect.left() + state_rect.width() * 0.45,
        state_rect.top() + 20.0 + 4.0 + 14.0 + 11.0,
    );
    harness.hover_at(hover_pos);
    harness.run();

    let highlight = harness.run_with_viewer_context(move |viewer_context| {
        viewer_context
            .time_ctrl
            .highlighted_range()
            .filter(|h| {
                h.timeline == timeline_name
                    && h.kind
                        == re_viewer::external::re_viewer_context::TimeRangeHighlightKind::StateTimeline
            })
            .cloned()
    });

    let highlight = highlight.expect(
        "hovering over a state phase should publish a StateTimeline TimeRangeHighlight via \
         TimeControl, so the time panel and other time-based views can render it",
    );
    // Hovered the middle phase: 40..80.
    assert_eq!(
        highlight.range.min.as_i64(),
        40,
        "highlight start should match the hovered phase's start tick (40 = 'Moving')",
    );
    assert_eq!(
        highlight.range.max.as_i64(),
        80,
        "highlight end should match the next phase's start tick (80 = 'Done')",
    );
    assert!(
        highlight.color.is_some(),
        "Data highlights from the state timeline view must carry a fill color",
    );

    harness.snapshot_app("state_timeline_hover_highlight_after");
}
