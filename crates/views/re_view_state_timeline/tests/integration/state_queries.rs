//! Tests for state timeline window queries, bootstrapping, and visualizer diagnostics.

use std::sync::Arc;

use re_log_types::Timeline;
use re_log_types::external::arrow::array::StringArray;
use re_sdk_types::archetypes::{StateChange, StateConfiguration};
use re_sdk_types::{DynamicArchetype, Visualizer};
use re_test_context::TestContext;
use re_view_state_timeline::{StateLanesOutput, StateTimelineView, StateVisualizer};
use re_viewer_context::{IdentifiedViewSystem as _, ViewId};

use super::common::{
    self, build_view, map_source_to_state, run_visualizer_systems, timed_phase_labels,
};

fn run_visualizer_with_window(
    test_context: &TestContext,
    view_id: ViewId,
    min: f64,
    time_spanned: f64,
) -> Vec<StateLanesOutput> {
    common::run_visualizer_data(test_context, view_id, Some((min, time_spanned)))
}

/// A null row before the visible window is a reset. With the window panned to
/// `[25, 35]` and data `Idle@0`, `[null]@20`, `Active@40`, the lane shows a gap at the
/// window's left edge: the single latest-at bootstrap row (the null) fully describes the
/// state there — no further look-back is needed.
#[test]
fn test_null_before_window_resets_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/null_before_window";

    for (tick, array) in [
        (0i64, StringArray::from(vec![Some("Idle")])),
        (20, StringArray::from(vec![None::<&str>])),
        (40, StringArray::from(vec![Some("Active")])),
    ] {
        let archetype = DynamicArchetype::new("strings")
            .with_component_from_data("value", Arc::new(array) as Arc<_>);
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state("strings:value")],
    );

    let outputs = run_visualizer_with_window(&test_context, view_id, 25.0, 10.0);
    assert_eq!(outputs.len(), 1);
    // The bootstrap yields the gap event from the null@20; being a leading gap it is
    // dropped, leaving the lane empty until `Active`@40 (just past the window).
    assert_eq!(
        timed_phase_labels(&outputs[0], entity),
        vec![(40, "Active".to_owned())]
    );
}

// Regression test for RR-5673
#[test]
fn test_state_and_configuration_before_window_do_not_report_warnings() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/before_window";
    test_context.log_entity(entity, |builder| {
        builder.with_archetype_auto_row([(Timeline::log_tick(), 40)], &StateChange::single("Idle"))
    });
    test_context.log_entity(entity, |builder| {
        builder.with_archetype_auto_row(
            [(Timeline::log_tick(), 40)],
            &StateConfiguration::new()
                .with_values(["Idle"])
                .with_labels(["At rest"])
                .with_colors([0x4CAF50FFu32])
                .with_visible([true]),
        )
    });
    let view_id = build_view(
        &mut test_context,
        entity,
        [Visualizer::new(StateVisualizer::identifier().as_str())],
    );
    test_context.set_time(re_log_types::TimeInt::new_temporal(100));

    let output = run_visualizer_systems(&test_context, view_id, Some((100.0, 100.0)));
    let result = &output.visualizer_execution_output.per_visualizer[&StateVisualizer::identifier()];

    // There should be no error report (converting to VisualizerTypeReport since it's printable)
    let report = re_viewer_context::VisualizerTypeReport::from_result(result);
    assert!(
        report.is_none(),
        "unexpected visualizer diagnostics: {report:#?}"
    );

    // Make sure we actually produced output data.
    let lanes = output
        .visualizer_data::<StateLanesOutput>(StateVisualizer::identifier())
        .expect("state visualizer should succeed")
        .expect("state visualizer should produce lanes");
    assert_eq!(
        timed_phase_labels(lanes, entity),
        vec![(40, "At rest".to_owned())]
    );
}

/// The state before the visible window is merged with its original index, so it remains visible
/// and its original time remains available for the tooltip.
#[test]
fn test_bootstrapped_state_keeps_its_time_outside_the_window() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/bootstrap_preserves_time_only";
    let archetype = DynamicArchetype::new("states")
        .with_component_from_data("value", Arc::new(StringArray::from(vec!["Idle"])) as Arc<_>);
    test_context.log_entity(entity, |builder| {
        builder.with_archetype_auto_row([(Timeline::log_tick(), 40)], &archetype)
    });

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state("states:value")],
    );
    let outputs = run_visualizer_with_window(&test_context, view_id, 100.0, 100.0);

    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], entity),
        vec![(40, "Idle".to_owned())]
    );
}

/// Same-timestamp sibling rows — the later row id wins, both in-window and at the
/// window-edge bootstrap: `Idle`@20 then `[null]`@20 means the state at t=20 is reset, so a
/// window starting after 20 shows a gap until the next state.
#[test]
fn test_null_wins_over_same_time_sibling_at_bootstrap() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/null_same_time";

    for (tick, array) in [
        (20i64, StringArray::from(vec![Some("Idle")])),
        (20, StringArray::from(vec![None::<&str>])),
        (40, StringArray::from(vec![Some("Active")])),
    ] {
        let archetype = DynamicArchetype::new("strings")
            .with_component_from_data("value", Arc::new(array) as Arc<_>);
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state("strings:value")],
    );

    let outputs = run_visualizer_with_window(&test_context, view_id, 25.0, 10.0);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], entity),
        vec![(40, "Active".to_owned())]
    );
}
