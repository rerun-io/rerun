//! Integration tests for dropping components from the streams tree onto a time series view.
//!
//! These exercise [`re_viewer_context::ViewClass::handle_component_drop`] end-to-end: the
//! drop-feedback gating (which components are accepted) and the resulting blueprint mutation.

use std::sync::Arc;

use re_log_types::external::arrow::array::{
    ArrayRef, BooleanArray, Int64Array, ListArray, StringArray,
};
use re_log_types::external::arrow::buffer::OffsetBuffer;
use re_log_types::external::arrow::datatypes::{DataType, Field};
use re_log_types::{ComponentPath, EntityPath};
use re_sdk_types::DynamicArchetype;
use re_sdk_types::archetypes::Scalars;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{
    DragAndDropFeedback, RecommendedView, ViewClass as _, ViewId, ViewerContext,
};
use re_viewport_blueprint::ViewBlueprint;

/// An entity the view doesn't contain, so dropped components are always "new".
const OTHER_ENTITY: &str = "plots/other";

/// A list-typed component column holding a single `Int64` row — a plottable but non-float scalar.
fn int64_component(value: i64) -> ArrayRef {
    let values = Arc::new(Int64Array::from(vec![value])) as ArrayRef;
    let offsets = OffsetBuffer::from_lengths([1usize]);
    Arc::new(ListArray::new(
        Arc::new(Field::new("item", DataType::Int64, false)),
        offsets,
        values,
        None,
    ))
}

/// A list-typed component column holding a single `Boolean` row — a plottable scalar.
fn bool_component(value: bool) -> ArrayRef {
    let values = Arc::new(BooleanArray::from(vec![value])) as ArrayRef;
    let offsets = OffsetBuffer::from_lengths([1usize]);
    Arc::new(ListArray::new(
        Arc::new(Field::new("item", DataType::Boolean, false)),
        offsets,
        values,
        None,
    ))
}

/// A list-typed component column holding a single `Utf8` row — not plottable.
fn utf8_component(value: &str) -> ArrayRef {
    let values = Arc::new(StringArray::from(vec![value])) as ArrayRef;
    let offsets = OffsetBuffer::from_lengths([1usize]);
    Arc::new(ListArray::new(
        Arc::new(Field::new("item", DataType::Utf8, false)),
        offsets,
        values,
        None,
    ))
}

/// The single component logged on `entity`, as a [`ComponentPath`] suitable for a drop.
fn sole_component(ctx: &ViewerContext<'_>, entity: &EntityPath) -> ComponentPath {
    let engine = ctx.recording().storage_engine();
    let components = engine
        .store()
        .schema()
        .all_components_for_entity(entity)
        .expect("entity should have logged data")
        .clone();
    assert_eq!(
        components.len(),
        1,
        "test entity {entity} should have exactly one component, got {components:?}"
    );
    let component = *components.iter().next().expect("exactly one component");
    ComponentPath::new(entity.clone(), component)
}

/// A view that only contains [`OTHER_ENTITY`], so every other entity is a fresh drop target.
fn setup_view_excluding_dropped_entities(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            TimeSeriesView::identifier(),
            RecommendedView::new_single_entity(EntityPath::from(OTHER_ENTITY)),
        ))
    })
}

/// Dropping a fresh scalar component is accepted and adds a visualizer to the view.
#[test]
fn test_drop_scalar_component_adds_visualizer() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = test_context.active_timeline().expect("active timeline");

    let entity = EntityPath::from("plots/sin");
    for i in 0..5 {
        let t = i as f64 / 8.0;
        test_context.log_entity(entity.clone(), |builder| {
            builder.with_archetype_auto_row([(timeline, i)], &Scalars::single(t.sin()))
        });
    }

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    // Precondition: the entity is not part of the view yet.
    assert!(
        test_context
            .query_results
            .get(&view_id)
            .and_then(|qr| qr.tree.lookup_result_by_path(entity.hash()))
            .is_none(),
        "entity should not be visualized before the drop"
    );

    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let component_path = sole_component(ctx, &entity);
        let feedback = TimeSeriesView.handle_component_drop(
            ctx,
            view_id,
            &[component_path],
            /* released */ true,
        );
        assert_eq!(feedback, DragAndDropFeedback::Accept);
    });
    test_context.handle_system_commands(&egui::Context::default());

    // Recompute query results against the mutated blueprint.
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    let data_result = test_context
        .query_results
        .get(&view_id)
        .expect("view has query results")
        .tree
        .lookup_result_by_path(entity.hash())
        .cloned();
    assert!(
        data_result.is_some_and(|r| !r.visualizer_instructions.is_empty()),
        "the dropped entity should now have a visualizer in the view"
    );
}

/// Dropping a non-scalar component (e.g. a string) is rejected as incompatible.
#[test]
fn test_drop_non_scalar_component_is_rejected() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = test_context.active_timeline().expect("active timeline");

    let entity = EntityPath::from("plots/text");
    for i in 0..5 {
        test_context.log_entity(entity.clone(), |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("data")
                    .with_component_from_data("value", utf8_component("hi")),
            )
        });
    }

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let component_path = sole_component(ctx, &entity);
        let feedback = TimeSeriesView.handle_component_drop(
            ctx,
            view_id,
            &[component_path],
            /* released */ false,
        );
        assert_eq!(
            feedback,
            DragAndDropFeedback::Reject(Some("Not a scalar component"))
        );
    });
}

/// Regression test: integer (non-float) scalars are plottable and must be accepted, even though
/// they aren't among the *recommended* (float) datatypes used for spawn heuristics.
#[test]
fn test_drop_integer_scalar_component_is_accepted() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = test_context.active_timeline().expect("active timeline");

    let entity = EntityPath::from("plots/ints");
    for i in 0..5 {
        test_context.log_entity(entity.clone(), |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("data")
                    .with_component_from_data("value", int64_component(i)),
            )
        });
    }

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let component_path = sole_component(ctx, &entity);
        let feedback = TimeSeriesView.handle_component_drop(
            ctx,
            view_id,
            &[component_path],
            /* released */ false,
        );
        assert_eq!(feedback, DragAndDropFeedback::Accept);
    });
}

/// Boolean scalars are plottable (0/1) and must be accepted, like integers.
#[test]
fn test_drop_bool_scalar_component_is_accepted() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = test_context.active_timeline().expect("active timeline");

    let entity = EntityPath::from("plots/flag");
    for i in 0..5 {
        test_context.log_entity(entity.clone(), |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("data")
                    .with_component_from_data("value", bool_component(i % 2 == 0)),
            )
        });
    }

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let component_path = sole_component(ctx, &entity);
        let feedback = TimeSeriesView.handle_component_drop(
            ctx,
            view_id,
            &[component_path],
            /* released */ false,
        );
        assert_eq!(feedback, DragAndDropFeedback::Accept);
    });
}
