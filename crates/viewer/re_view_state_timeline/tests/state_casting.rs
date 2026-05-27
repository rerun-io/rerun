//! Tests for the polymorphic state cast: how different physical types arriving at
//! the `StateChange:state` slot are canonicalized into a [`StateValueKind`] and
//! formatted into phase labels.
//!
//! Coverage:
//! - Casting various physical types via `DynamicArchetype` (Int, Float, Bool, String).
//! - A `DynamicArchetype` with multiple state-like components — same type and mixed types.
//! - Using a real `TextLog` archetype as the source for the state slot.

use std::sync::Arc;

use re_log_types::external::arrow::array::{BooleanArray, Float64Array, Int32Array, StringArray};
use re_log_types::{EntityPath, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};
use re_sdk_types::{DynamicArchetype, Visualizer};
use re_test_context::TestContext;
use re_test_context::VisualizerBlueprintContext as _;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::{StateLanesData, StateTimelineView, StateValueKind, StateVisualizer};
use re_viewer_context::{IdentifiedViewSystem as _, ViewClass as _, ViewId};
use re_viewport::execute_systems_for_view;
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

const STATE_TARGET: &str = "StateChange:state";

/// Map a custom source component onto the `StateChange:state` slot of a `StateVisualizer`.
///
/// `save_visualizers` bypasses the default auto-spawn heuristics, which only fire when the
/// entity is indicated for the `StateChange` archetype. Custom archetypes (`DynamicArchetype`,
/// `TextLog`) are not indicated, so the visualizer instruction has to be installed explicitly.
fn map_source_to_state(source_component: &str) -> Visualizer {
    Visualizer::new(StateVisualizer::identifier().as_str()).with_mappings([
        VisualizerComponentMapping {
            target: STATE_TARGET.into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some(source_component.into()),
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
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let viewport_blueprint =
            ViewportBlueprint::from_db(ctx.store_context.blueprint, &test_context.blueprint_query);
        let view_blueprint = viewport_blueprint
            .view(&view_id)
            .expect("view should exist in blueprint");

        let class_registry = ctx.view_class_registry();
        let view_class = class_registry.get_class_or_log_error(view_blueprint.class_identifier());
        let view_state = view_class.new_state();

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
    archetype_name: &str,
    field_name: &str,
    arrays: [F; 3],
) -> ViewId
where
    F: Into<re_log_types::external::arrow::array::ArrayRef>,
{
    let source_component = format!("{archetype_name}:{field_name}");

    for (tick, array) in (0..3i64).zip(arrays) {
        let archetype = DynamicArchetype::new(archetype_name)
            .with_component_from_data(field_name, array.into());
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    build_view(
        test_context,
        entity,
        [map_source_to_state(&source_component)],
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

/// A `DynamicArchetype` carrying two fields of the same physical type. Mapping each as a
/// separate state source yields two lanes on the same entity.
#[test]
fn test_dynamic_archetype_multiple_same_type() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/multi_same";

    for (tick, (a, b)) in (0..3i64).zip([("Idle", "Off"), ("Active", "On"), ("Idle", "On")]) {
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

    for (tick, (s, f, b)) in (0..3i64).zip(frames) {
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
