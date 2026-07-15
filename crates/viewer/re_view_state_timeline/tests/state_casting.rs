//! Tests for the polymorphic state cast: how different physical types arriving at
//! the `StateChange:state` slot are canonicalized into a [`StateValueKind`] and
//! formatted into phase labels.
//!
//! Coverage:
//! - Casting various physical types via `DynamicArchetype` (Int, Float, Bool, String).
//! - A `DynamicArchetype` with multiple state-like components — same type and mixed types.
//! - Using a real `TextLog` archetype as the source for the state slot.

use std::sync::Arc;

use re_log_types::external::arrow::array::{
    BooleanArray, Float64Array, Int32Array, LargeStringArray, StringArray,
};
use re_log_types::{EntityPath, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};
use re_sdk_types::{ArchetypeName, ComponentIdentifier, DynamicArchetype, Visualizer};
use re_test_context::TestContext;
use re_test_context::VisualizerBlueprintContext as _;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::{
    StateLanesData, StateTimelineView, StateTimelineViewState, StateValueKind, StateVisualizer,
};
use re_viewer_context::{IdentifiedViewSystem as _, ViewClass as _, ViewId};
use re_viewport::execute_systems_for_view;
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

const STATE_TARGET: &str = "StateChange:state";

/// Map a custom source component onto the `StateChange:state` slot of a `StateVisualizer`.
///
/// `save_visualizers` bypasses the default auto-spawn heuristics, which only fire when the
/// entity is indicated for the `StateChange` archetype. Custom archetypes (`DynamicArchetype`,
/// `TextLog`) are not indicated, so the visualizer instruction has to be installed explicitly.
fn map_source_to_state(source_component: impl Into<ComponentIdentifier>) -> Visualizer {
    let source_component = source_component.into();
    Visualizer::new(StateVisualizer::identifier().as_str()).with_mappings([
        VisualizerComponentMapping {
            target: STATE_TARGET.into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some(source_component.as_str().into()),
            selector: None,
        }
        .into(),
    ])
}

/// Lock in the [`Timeline::log_tick`] timeline as active and set up a viewport blueprint
/// with a single view that maps the given visualizers onto `entity`.
///
/// Must be called *after* data has been logged: `set_active_timeline` reads the entity DB
/// when it runs, so the timeline only resolves to a concrete [`Timeline`] (rather than
/// staying [`re_viewer_context::ActiveTimeline::Pending`]) once some data exists on it.
/// After this, [`TestContext::active_timeline`] returns `Some(Timeline::log_tick())`.
fn build_view(
    test_context: &mut TestContext,
    entity: &str,
    visualizers: impl IntoIterator<Item = Visualizer>,
) -> ViewId {
    test_context.set_active_timeline(*Timeline::log_tick().name());

    let visualizers: Vec<_> = visualizers.into_iter().collect();
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(StateTimelineView::identifier());
        ctx.save_visualizers(&EntityPath::from(entity), view.id, visualizers);
        blueprint.add_view_at_root(view)
    })
}

/// Run the state visualizer and collect every emitted [`StateLanesData`].
///
/// Reconstructs the per-frame execution that the viewport normally performs, then peeks at
/// the visualizer's typed output rather than rendering it.
fn run_visualizer(test_context: &TestContext, view_id: ViewId) -> Vec<StateLanesData> {
    run_visualizer_impl(test_context, view_id, None)
}

/// Like [`run_visualizer`] but with the visible window constrained to
/// `[min, min + time_spanned]`, as if the user had panned/zoomed there.
fn run_visualizer_with_window(
    test_context: &TestContext,
    view_id: ViewId,
    min: f64,
    time_spanned: f64,
) -> Vec<StateLanesData> {
    run_visualizer_impl(test_context, view_id, Some((min, time_spanned)))
}

fn run_visualizer_impl(
    test_context: &TestContext,
    view_id: ViewId,
    window: Option<(f64, f64)>,
) -> Vec<StateLanesData> {
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let viewport_blueprint =
            ViewportBlueprint::from_db(ctx.store_context.blueprint, &test_context.blueprint_query);
        let view_blueprint = viewport_blueprint
            .view(&view_id)
            .expect("view should exist in blueprint");

        let class_registry = ctx.view_class_registry();
        let view_class = class_registry.get_class_or_log_error(view_blueprint.class_identifier());
        let mut view_state = view_class.new_state();

        if let Some((min, time_spanned)) = window {
            view_state
                .as_any_mut()
                .downcast_mut::<StateTimelineViewState>()
                .expect("state timeline view state")
                .time_views
                .insert(
                    *Timeline::log_tick().name(),
                    re_viewer_context::TimeView {
                        min: min.into(),
                        time_spanned,
                    },
                );
        }

        let once_per_frame = class_registry.run_once_per_frame_context_systems(
            ctx,
            std::iter::once(view_blueprint.class_identifier()),
        );

        let (_view_query, system_output) =
            execute_systems_for_view(ctx, view_blueprint, view_state.as_ref(), &once_per_frame);

        system_output
            .iter_visualizer_data::<StateLanesData>()
            .cloned()
            .collect()
    })
}

fn phase_labels(lanes_data: &StateLanesData, entity: &str) -> Vec<String> {
    let lane = lanes_data
        .lanes
        .iter()
        .find(|l| l.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane for entity {entity}"));
    lane.phases
        .iter()
        .map(|p| {
            p.content
                .as_ref()
                .map_or_else(String::new, |s| s.label.clone())
        })
        .collect()
}

/// Like [`phase_labels`] but keeps each phase's start time, so tests can assert *when* a
/// reset (gap, rendered as an empty label) begins.
fn timed_phase_labels(lanes_data: &StateLanesData, entity: &str) -> Vec<(i64, String)> {
    let lane = lanes_data
        .lanes
        .iter()
        .find(|l| l.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane for entity {entity}"));
    lane.phases
        .iter()
        .map(|p| {
            (
                p.start_time,
                p.content
                    .as_ref()
                    .map_or_else(String::new, |s| s.label.clone()),
            )
        })
        .collect()
}

fn value_kind(lanes_data: &StateLanesData, entity: &str) -> StateValueKind {
    let lane = lanes_data
        .lanes
        .iter()
        .find(|l| l.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane for entity {entity}"));
    lane.value_kind
}

/// Log a `DynamicArchetype` with one field at three ticks, then install an explicit visualizer
/// mapping from that field to `StateChange:state`. Returns the view id.
fn setup_single_field<F>(
    test_context: &mut TestContext,
    entity: &str,
    archetype_name: impl Into<ArchetypeName>,
    field_name: &str,
    arrays: [F; 3],
) -> ViewId
where
    F: Into<re_log_types::external::arrow::array::ArrayRef>,
{
    let archetype = archetype_name.into();
    let source_component = ComponentIdentifier::from_archetype_field(archetype, field_name);

    for (tick, array) in std::iter::zip(0..3i64, arrays) {
        let dyn_archetype = DynamicArchetype::new(archetype).with_component_from_data(
            ComponentIdentifier::try_new(field_name).expect("valid component"),
            array.into(),
        );
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &dyn_archetype)
        });
    }

    build_view(
        test_context,
        entity,
        [map_source_to_state(source_component)],
    )
}

#[test]
fn test_cast_int32_via_dynamic_archetype() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/int",
        "ints",
        "value",
        [
            Arc::new(Int32Array::from(vec![1])) as Arc<_>,
            Arc::new(Int32Array::from(vec![2])) as Arc<_>,
            Arc::new(Int32Array::from(vec![1])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1, "expected one StateLanesData output");

    // Int32 collapses to Float64; integer-valued floats render without a trailing `.0`,
    // and consecutive identical phases merge.
    assert_eq!(
        value_kind(&outputs[0], "/state/int"),
        StateValueKind::Scalar
    );
    assert_eq!(phase_labels(&outputs[0], "/state/int"), vec!["1", "2", "1"]);

    test_context
        .run_view_ui_and_save_snapshot(view_id, "state_cast_int32", egui::vec2(400.0, 80.0), None)
        .unwrap();
}

#[test]
fn test_cast_float64_via_dynamic_archetype() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/float",
        "floats",
        "value",
        [
            Arc::new(Float64Array::from(vec![1.5])) as Arc<_>,
            Arc::new(Float64Array::from(vec![2.0])) as Arc<_>,
            Arc::new(Float64Array::from(vec![2.0])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        value_kind(&outputs[0], "/state/float"),
        StateValueKind::Scalar
    );
    // Non-integer floats keep their fractional part; integer-valued floats drop the `.0`.
    // The trailing duplicate `2.0` merges with the previous phase.
    assert_eq!(phase_labels(&outputs[0], "/state/float"), vec!["1.5", "2"]);

    test_context
        .run_view_ui_and_save_snapshot(view_id, "state_cast_float64", egui::vec2(400.0, 80.0), None)
        .unwrap();
}

#[test]
fn test_cast_bool_via_dynamic_archetype() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/bool",
        "bools",
        "value",
        [
            Arc::new(BooleanArray::from(vec![false])) as Arc<_>,
            Arc::new(BooleanArray::from(vec![true])) as Arc<_>,
            Arc::new(BooleanArray::from(vec![false])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(value_kind(&outputs[0], "/state/bool"), StateValueKind::Bool);
    assert_eq!(
        phase_labels(&outputs[0], "/state/bool"),
        vec!["false", "true", "false"]
    );

    test_context
        .run_view_ui_and_save_snapshot(view_id, "state_cast_bool", egui::vec2(400.0, 80.0), None)
        .unwrap();
}

#[test]
fn test_cast_string_via_dynamic_archetype() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/string",
        "strings",
        "value",
        [
            Arc::new(StringArray::from(vec!["idle"])) as Arc<_>,
            Arc::new(StringArray::from(vec!["active"])) as Arc<_>,
            Arc::new(StringArray::from(vec!["idle"])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        value_kind(&outputs[0], "/state/string"),
        StateValueKind::String
    );
    assert_eq!(
        phase_labels(&outputs[0], "/state/string"),
        vec!["idle", "active", "idle"]
    );

    test_context
        .run_view_ui_and_save_snapshot(view_id, "state_cast_string", egui::vec2(400.0, 80.0), None)
        .unwrap();
}

/// A null value resets a scalar lane, ending the current phase and leaving a gap
/// (rendered here as an empty label) until the next non-null value.
#[test]
fn test_null_resets_float_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/float_null",
        "floats",
        "value",
        [
            Arc::new(Float64Array::from(vec![Some(1.5)])) as Arc<_>,
            Arc::new(Float64Array::from(vec![None])) as Arc<_>,
            Arc::new(Float64Array::from(vec![Some(2.5)])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/float_null"),
        vec![
            (0, "1.5".to_owned()),
            (1, String::new()),
            (2, "2.5".to_owned())
        ]
    );
}

/// The Int32 → Float64 state cast must preserve nulls, so a null integer also
/// resets the lane.
#[test]
fn test_null_resets_int_lane_via_cast() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/int_null",
        "ints",
        "value",
        [
            Arc::new(Int32Array::from(vec![Some(1)])) as Arc<_>,
            Arc::new(Int32Array::from(vec![None])) as Arc<_>,
            Arc::new(Int32Array::from(vec![Some(2)])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/int_null"),
        vec![(0, "1".to_owned()), (1, String::new()), (2, "2".to_owned())]
    );
}

/// A null value resets a bool lane.
#[test]
fn test_null_resets_bool_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/bool_null",
        "bools",
        "value",
        [
            Arc::new(BooleanArray::from(vec![Some(true)])) as Arc<_>,
            Arc::new(BooleanArray::from(vec![None])) as Arc<_>,
            Arc::new(BooleanArray::from(vec![Some(false)])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/bool_null"),
        vec![
            (0, "true".to_owned()),
            (1, String::new()),
            (2, "false".to_owned())
        ]
    );
}

/// A null value resets a string lane, just like an explicitly-empty string.
#[test]
fn test_null_resets_string_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/string_null",
        "strings",
        "value",
        [
            Arc::new(StringArray::from(vec![Some("idle")])) as Arc<_>,
            Arc::new(StringArray::from(vec![None::<&str>])) as Arc<_>,
            Arc::new(StringArray::from(vec![Some("active")])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/string_null"),
        vec![
            (0, "idle".to_owned()),
            (1, String::new()),
            (2, "active".to_owned())
        ]
    );
}

/// `LargeUtf8` source data behaves like `Utf8`: values render and nulls reset.
#[test]
fn test_null_resets_large_string_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/large_string_null",
        "large_strings",
        "value",
        [
            Arc::new(LargeStringArray::from(vec![Some("idle")])) as Arc<_>,
            Arc::new(LargeStringArray::from(vec![None::<&str>])) as Arc<_>,
            Arc::new(LargeStringArray::from(vec![Some("active")])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        value_kind(&outputs[0], "/state/large_string_null"),
        StateValueKind::String
    );
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/large_string_null"),
        vec![
            (0, "idle".to_owned()),
            (1, String::new()),
            (2, "active".to_owned())
        ]
    );
}

/// An empty state batch (a row with zero values, e.g. from `clear_fields`) resets a
/// scalar lane, matching `Clear` and latest-at clear semantics.
#[test]
fn test_empty_batch_resets_float_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/float_empty",
        "floats",
        "value",
        [
            Arc::new(Float64Array::from(vec![1.5])) as Arc<_>,
            Arc::new(Float64Array::from(Vec::<f64>::new())) as Arc<_>,
            Arc::new(Float64Array::from(vec![2.5])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/float_empty"),
        vec![
            (0, "1.5".to_owned()),
            (1, String::new()),
            (2, "2.5".to_owned())
        ]
    );
}

/// An empty state batch resets a string lane.
#[test]
fn test_empty_batch_resets_string_lane() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let view_id = setup_single_field(
        &mut test_context,
        "/state/string_empty",
        "strings",
        "value",
        [
            Arc::new(StringArray::from(vec!["idle"])) as Arc<_>,
            Arc::new(StringArray::from(Vec::<&str>::new())) as Arc<_>,
            Arc::new(StringArray::from(vec!["active"])) as Arc<_>,
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        timed_phase_labels(&outputs[0], "/state/string_empty"),
        vec![
            (0, "idle".to_owned()),
            (1, String::new()),
            (2, "active".to_owned())
        ]
    );
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

/// A `DynamicArchetype` carrying two fields of the same physical type. Mapping each as a
/// separate state source yields two lanes on the same entity.
#[test]
fn test_dynamic_archetype_multiple_same_type() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/multi_same";

    for (tick, (a, b)) in
        std::iter::zip(0..3i64, [("Idle", "Off"), ("Active", "On"), ("Idle", "On")])
    {
        let archetype = DynamicArchetype::new("multi_str")
            .with_component_from_data("mode", Arc::new(StringArray::from(vec![a])))
            .with_component_from_data("power", Arc::new(StringArray::from(vec![b])));
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [
            map_source_to_state("multi_str:mode"),
            map_source_to_state("multi_str:power"),
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);

    // One lane per visualizer instruction; both lanes share the same entity path.
    let lanes_on_entity: Vec<_> = outputs[0]
        .lanes
        .iter()
        .filter(|l| l.entity_path == EntityPath::from(entity))
        .collect();
    assert_eq!(lanes_on_entity.len(), 2);

    for lane in &lanes_on_entity {
        assert_eq!(lane.value_kind, StateValueKind::String);
    }

    // The lane label disambiguates which source field is feeding this lane.
    let mode_lane = lanes_on_entity
        .iter()
        .find(|l| l.label.contains("multi_str:mode"))
        .expect("expected a lane sourced from multi_str:mode");
    let power_lane = lanes_on_entity
        .iter()
        .find(|l| l.label.contains("multi_str:power"))
        .expect("expected a lane sourced from multi_str:power");

    let phase_label = |p: &re_view_state_timeline::StateLanePhase| {
        p.content
            .as_ref()
            .map_or_else(String::new, |s| s.label.clone())
    };
    let mode_labels: Vec<_> = mode_lane.phases.iter().map(phase_label).collect();
    let power_labels: Vec<_> = power_lane.phases.iter().map(phase_label).collect();
    assert_eq!(mode_labels, vec!["Idle", "Active", "Idle"]);
    // "On" at ticks 1 and 2 merge into a single phase.
    assert_eq!(power_labels, vec!["Off", "On"]);

    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "state_cast_multi_same_type",
            egui::vec2(400.0, 150.0),
            None,
        )
        .unwrap();
}

/// A `DynamicArchetype` carrying three fields of different physical types. Each mapping
/// produces a lane whose `value_kind` matches the post-cast type.
#[test]
fn test_dynamic_archetype_multiple_different_types() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/multi_mixed";

    let frames = [
        ("idle", 0.0_f64, false),
        ("running", 1.0_f64, true),
        ("idle", 0.0_f64, false),
    ];

    for (tick, (s, f, b)) in std::iter::zip(0..3i64, frames) {
        let archetype = DynamicArchetype::new("multi_mix")
            .with_component_from_data("label", Arc::new(StringArray::from(vec![s])))
            .with_component_from_data("speed", Arc::new(Float64Array::from(vec![f])))
            .with_component_from_data("on", Arc::new(BooleanArray::from(vec![b])));
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [
            map_source_to_state("multi_mix:label"),
            map_source_to_state("multi_mix:speed"),
            map_source_to_state("multi_mix:on"),
        ],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    let lanes = &outputs[0].lanes;
    assert_eq!(lanes.len(), 3);

    let kind_of = |source: &str| {
        lanes
            .iter()
            .find(|l| l.label.contains(source))
            .unwrap_or_else(|| panic!("no lane labelled with {source}"))
            .value_kind
    };
    assert_eq!(kind_of("multi_mix:label"), StateValueKind::String);
    assert_eq!(kind_of("multi_mix:speed"), StateValueKind::Scalar);
    assert_eq!(kind_of("multi_mix:on"), StateValueKind::Bool);

    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "state_cast_multi_different_types",
            egui::vec2(400.0, 200.0),
            None,
        )
        .unwrap();
}

/// `TextLog` is a real Rerun archetype with a `text` string field. Mapping that field as
/// the state source should produce a string-kind lane carrying the logged messages.
#[test]
fn test_textlog_archetype_visualized_as_string() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/log";

    for (tick, message) in [(0_i64, "starting"), (1, "ready"), (2, "stopping")] {
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &TextLog::new(message))
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state(
            TextLog::descriptor_text().component.as_str(),
        )],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(value_kind(&outputs[0], entity), StateValueKind::String);
    assert_eq!(
        phase_labels(&outputs[0], entity),
        vec!["starting", "ready", "stopping"]
    );

    test_context
        .run_view_ui_and_save_snapshot(view_id, "state_cast_textlog", egui::vec2(400.0, 80.0), None)
        .unwrap();
}

/// When the underlying column's physical type changes over time, the polymorphic cast hands
/// back chunks with mixed element types and slicing them as a single type would
/// `debug_panic!` in `re_chunk::iter`. The visualizer must detect this and skip the lane
/// rather than panic.
#[test]
fn test_mixed_chunk_types_do_not_panic() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/mixed";

    // First a Utf8 chunk, then a Boolean chunk under the same component identifier.
    test_context.log_entity(entity, |builder| {
        builder.with_archetype_auto_row(
            [(Timeline::log_tick(), 0_i64)],
            &DynamicArchetype::new("mixed")
                .with_component_from_data("value", Arc::new(StringArray::from(vec!["ponies"]))),
        )
    });
    test_context.log_entity(entity, |builder| {
        builder.with_archetype_auto_row(
            [(Timeline::log_tick(), 1_i64)],
            &DynamicArchetype::new("mixed")
                .with_component_from_data("value", Arc::new(BooleanArray::from(vec![true]))),
        )
    });

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state("mixed:value")],
    );

    // The visualizer must run to completion (no panic) and emit no lane for this entity.
    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert!(
        outputs[0]
            .lanes
            .iter()
            .all(|l| l.entity_path != EntityPath::from(entity)),
        "expected no lane for the mixed-type entity, got: {:?}",
        outputs[0].lanes
    );
}
