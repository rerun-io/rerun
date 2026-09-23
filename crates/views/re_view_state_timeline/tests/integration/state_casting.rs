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
use re_sdk_types::{ArchetypeName, ComponentIdentifier, DynamicArchetype};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::{StateLanesOutput, StateTimelineView, StateValueKind};
use re_viewer_context::ViewId;

use super::common::{
    self, build_view, map_source_to_state, map_source_to_state_with_selector, timed_phase_labels,
};

fn run_visualizer(test_context: &TestContext, view_id: ViewId) -> Vec<StateLanesOutput> {
    common::run_visualizer_data(test_context, view_id, None)
}

fn phase_labels(lanes_data: &StateLanesOutput, entity: &str) -> Vec<String> {
    let group = lanes_data
        .groups
        .iter()
        .find(|g| g.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane group for entity {entity}"));
    assert_eq!(
        group.lanes.len(),
        1,
        "expected a single-instance lane group for entity {entity}"
    );
    group.lanes[0]
        .phases
        .iter()
        .map(|p| {
            p.content
                .as_ref()
                .map_or_else(String::new, |s| s.label.clone())
        })
        .collect()
}

fn value_kind(lanes_data: &StateLanesOutput, entity: &str) -> StateValueKind {
    let group = lanes_data
        .groups
        .iter()
        .find(|g| g.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane group for entity {entity}"));
    group
        .value_kind
        .unwrap_or_else(|| panic!("lane group for entity {entity} has no value kind"))
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
    assert_eq!(outputs.len(), 1, "expected one StateLanesOutput output");

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

    // One lane group per visualizer instruction; both groups share the same entity path.
    let groups_on_entity: Vec<_> = outputs[0]
        .groups
        .iter()
        .filter(|g| g.entity_path == EntityPath::from(entity))
        .collect();
    assert_eq!(groups_on_entity.len(), 2);

    for group in &groups_on_entity {
        assert_eq!(group.value_kind, Some(StateValueKind::String));
        assert_eq!(group.lanes.len(), 1);
    }

    // The group label disambiguates which source field is feeding this group.
    let mode_group = groups_on_entity
        .iter()
        .find(|g| g.label.contains("multi_str:mode"))
        .expect("expected a lane group sourced from multi_str:mode");
    let power_group = groups_on_entity
        .iter()
        .find(|g| g.label.contains("multi_str:power"))
        .expect("expected a lane group sourced from multi_str:power");

    let phase_label = |p: &re_view_state_timeline::StateLanePhase| {
        p.content
            .as_ref()
            .map_or_else(String::new, |s| s.label.clone())
    };
    let mode_labels: Vec<_> = mode_group.lanes[0].phases.iter().map(phase_label).collect();
    let power_labels: Vec<_> = power_group.lanes[0]
        .phases
        .iter()
        .map(phase_label)
        .collect();
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
    let groups = &outputs[0].groups;
    assert_eq!(groups.len(), 3);

    let kind_of = |source: &str| {
        groups
            .iter()
            .find(|g| g.label.contains(source))
            .unwrap_or_else(|| panic!("no lane group labelled with {source}"))
            .value_kind
            .unwrap_or_else(|| panic!("lane group labelled with {source} has no value kind"))
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

/// A `u8` field nested in a struct, picked via a selector (RR-5038): the polymorphic cast
/// rule must judge the post-selector element type (`UInt8` → `Float64`), not the struct's
/// datatype — otherwise the cast is skipped and the lane silently vanishes.
#[test]
fn test_cast_nested_struct_u8_field_via_selector() {
    use re_log_types::external::arrow::array::{ArrayRef, StructArray, UInt8Array};
    use re_log_types::external::arrow::datatypes::{DataType, Field};

    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let entity = "/state/nested_u8";

    for (tick, value) in [(0_i64, 1_u8), (1, 2), (2, 1)] {
        let struct_array = StructArray::from(vec![
            (
                Arc::new(Field::new("u8", DataType::UInt8, false)),
                Arc::new(UInt8Array::from(vec![value])) as ArrayRef,
            ),
            (
                Arc::new(Field::new("text", DataType::Utf8, false)),
                Arc::new(StringArray::from(vec!["ignored"])) as ArrayRef,
            ),
        ]);
        let archetype = DynamicArchetype::new("nested")
            .with_component_from_data("value", Arc::new(struct_array));
        test_context.log_entity(entity, |builder| {
            builder.with_archetype_auto_row([(Timeline::log_tick(), tick)], &archetype)
        });
    }

    let view_id = build_view(
        &mut test_context,
        entity,
        [map_source_to_state_with_selector(
            "nested:value",
            Some(".u8"),
        )],
    );

    let outputs = run_visualizer(&test_context, view_id);
    assert_eq!(outputs.len(), 1);
    assert_eq!(value_kind(&outputs[0], entity), StateValueKind::Scalar);
    assert_eq!(phase_labels(&outputs[0], entity), vec!["1", "2", "1"]);
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
            .groups
            .iter()
            .all(|g| g.entity_path != EntityPath::from(entity)),
        "expected no lane group for the mixed-type entity, got: {:?}",
        outputs[0].groups
    );
}
