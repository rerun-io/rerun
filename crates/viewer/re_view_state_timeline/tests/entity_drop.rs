//! Integration tests for dropping entities from the streams tree onto a state timeline view.
//!
//! These exercise the viewport's entity-drop path together with
//! [`re_viewer_context::ViewClass::reject_entity_drop_reason`]: which entities the view accepts,
//! and the resulting blueprint mutation.

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::DynamicArchetype;
use re_sdk_types::archetypes::{Scalars, StateChange};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{DragAndDropFeedback, RecommendedView, ViewClass as _, ViewId};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewBlueprint;

/// An entity the view contains from the start, so dropped entities are always "new".
const OTHER_ENTITY: &str = "state/other";

/// The reason `StateTimelineView` turns down an entity without state data.
const NO_STATE_DATA: &str =
    "No `StateChange` data on this entity, try dropping a component instead";

/// Log a `StateChange` on `entity` at a few ticks.
fn log_state_change(test_context: &mut TestContext, entity: &str, timeline: Timeline) {
    for (tick, state) in [(0, "Idle"), (10, "Moving"), (20, "Idle")] {
        test_context.log_entity(entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &StateChange::single(state),
            )
        });
    }
}

/// A view that only contains [`OTHER_ENTITY`], so every other entity is a fresh drop target.
fn setup_view_excluding_dropped_entities(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            StateTimelineView::identifier(),
            RecommendedView::new_single_entity(EntityPath::from(OTHER_ENTITY)),
        ))
    })
}

/// Drop `entities` onto `view_id` and return the feedback the viewport would show.
fn drop_entities(
    test_context: &TestContext,
    view_id: ViewId,
    entities: &[EntityPath],
    released: bool,
) -> DragAndDropFeedback {
    let mut feedback = None;
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let view_blueprint =
            ViewBlueprint::try_from_db(view_id, ctx.blueprint_db(), ctx.blueprint_query)
                .expect("view blueprint should exist");
        feedback = Some(ViewportUi::handle_drop_entities_to_view(
            ctx,
            &view_blueprint,
            entities,
            released,
        ));
    });
    test_context.handle_system_commands(&egui::Context::default());

    feedback.expect("the drop handler should have run")
}

/// Whether `entity` ended up in the view with a visualizer.
fn is_visualized(test_context: &TestContext, view_id: ViewId, entity: &EntityPath) -> bool {
    test_context
        .query_results
        .get(&view_id)
        .and_then(|query_result| query_result.tree.lookup_result_by_path(entity.hash()))
        .is_some_and(|data_result| !data_result.visualizer_instructions.is_empty())
}

/// Dropping an entity that logs `StateChange` is accepted and gives it a lane.
#[test]
fn test_drop_state_change_entity_adds_visualizer() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let entity = EntityPath::from("state/robot");
    log_state_change(&mut test_context, "state/robot", timeline);
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    // Precondition: the entity is not part of the view yet.
    assert!(
        !is_visualized(&test_context, view_id, &entity),
        "entity should not be visualized before the drop"
    );

    let feedback = drop_entities(
        &test_context,
        view_id,
        std::slice::from_ref(&entity),
        /* released */ true,
    );
    assert_eq!(feedback, DragAndDropFeedback::Accept);

    // Recompute query results against the mutated blueprint.
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    assert!(
        is_visualized(&test_context, view_id, &entity),
        "the dropped entity should now have a visualizer in the view"
    );
}

/// Dropping a parent of a state-change entity is accepted: the subtree rule brings the child in.
#[test]
fn test_drop_parent_of_state_change_entity_is_accepted() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let child = EntityPath::from("state/robot/arm");
    log_state_change(&mut test_context, "state/robot/arm", timeline);
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    let feedback = drop_entities(
        &test_context,
        view_id,
        &[EntityPath::from("state/robot")],
        /* released */ true,
    );
    assert_eq!(feedback, DragAndDropFeedback::Accept);

    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    assert!(
        is_visualized(&test_context, view_id, &child),
        "the state-change descendant should now have a visualizer in the view"
    );
}

/// A subtree brings nothing new when its only state-change entity is already in the view: the
/// entities that *would* be added have no state data, so the drop must be rejected rather than
/// silently widening the filter.
#[test]
fn test_drop_parent_whose_only_state_change_child_is_already_in_view_is_rejected() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let state_child = "state/robot/arm";
    log_state_change(&mut test_context, state_child, timeline);
    for tick in 0..5 {
        test_context.log_entity("state/robot/pose", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &Scalars::single(tick as f64),
            )
        });
    }
    test_context.set_active_timeline(*timeline.name());

    // A view of the state-change child alone — its scalar sibling is missing from the view.
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            StateTimelineView::identifier(),
            RecommendedView::new_single_entity(EntityPath::from(state_child)),
        ))
    });

    let feedback = drop_entities(
        &test_context,
        view_id,
        &[EntityPath::from("state/robot")],
        /* released */ false,
    );
    assert_eq!(feedback, DragAndDropFeedback::Reject(Some(NO_STATE_DATA)));
}

/// Merely hovering an acceptable entity over the view must not touch the blueprint.
#[test]
fn test_hovering_an_entity_does_not_mutate_the_view() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let entity = EntityPath::from("state/robot");
    log_state_change(&mut test_context, "state/robot", timeline);
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    let feedback = drop_entities(
        &test_context,
        view_id,
        std::slice::from_ref(&entity),
        /* released */ false,
    );
    assert_eq!(feedback, DragAndDropFeedback::Accept);

    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    assert!(
        !is_visualized(&test_context, view_id, &entity),
        "the entity was only hovered, so it should not have been added to the view"
    );
}

/// A drop of several entities goes through as long as one of them is acceptable, and only that
/// one is added.
#[test]
fn test_drop_of_mixed_entities_adds_only_the_state_change_one() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let state_entity = EntityPath::from("state/robot");
    log_state_change(&mut test_context, "state/robot", timeline);
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);

    let scalar_entity = EntityPath::from("plots/sin");
    for tick in 0..5 {
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &Scalars::single(tick as f64),
            )
        });
    }
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    let feedback = drop_entities(
        &test_context,
        view_id,
        &[scalar_entity.clone(), state_entity.clone()],
        /* released */ true,
    );
    assert_eq!(feedback, DragAndDropFeedback::Accept);

    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    assert!(is_visualized(&test_context, view_id, &state_entity));
    assert!(
        test_context
            .query_results
            .get(&view_id)
            .and_then(|query_result| query_result
                .tree
                .lookup_result_by_path(scalar_entity.hash()))
            .is_none(),
        "the entity without state data should not have been added to the view"
    );
}

/// Dropping an entity without a `StateChange` archetype is rejected, even though its scalar
/// component would satisfy the visualizer's (very permissive) datatype constraint.
#[test]
fn test_drop_entity_without_state_change_is_rejected() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let entity = EntityPath::from("plots/sin");
    for tick in 0..5 {
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &Scalars::single(tick as f64),
            )
        });
    }
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    let feedback = drop_entities(
        &test_context,
        view_id,
        std::slice::from_ref(&entity),
        /* released */ false,
    );
    assert_eq!(feedback, DragAndDropFeedback::Reject(Some(NO_STATE_DATA)));
}

/// Same for a string component that isn't tagged with the `StateChange` archetype: it could be
/// mapped onto the state slot by dropping the *component*, but dropping the entity is rejected
/// since nothing would tell the view which of its components to use.
#[test]
fn test_drop_entity_with_untagged_string_component_is_rejected() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let entity = EntityPath::from("logs/status");
    for tick in 0..5 {
        test_context.log_entity("logs/status", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &DynamicArchetype::new("data").with_component_from_data(
                    "value",
                    re_sdk_types::ComponentBatch::to_arrow(&re_sdk_types::components::Text::from(
                        "hi",
                    ))
                    .expect("text should serialize"),
                ),
            )
        });
    }
    log_state_change(&mut test_context, OTHER_ENTITY, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_view_excluding_dropped_entities(&mut test_context);

    let feedback = drop_entities(
        &test_context,
        view_id,
        std::slice::from_ref(&entity),
        /* released */ false,
    );
    assert_eq!(feedback, DragAndDropFeedback::Reject(Some(NO_STATE_DATA)));
}

/// The state timeline spawns as a root-wildcard view, which already contains every entity. Drops
/// onto it are still rejected, but with a reason for each case rather than a mute "no drop".
#[test]
fn test_drop_onto_root_wildcard_view_is_explained() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();

    let state_entity = EntityPath::from("state/robot");
    log_state_change(&mut test_context, "state/robot", timeline);

    let scalar_entity = EntityPath::from("plots/sin");
    for tick in 0..5 {
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &Scalars::single(tick as f64),
            )
        });
    }
    test_context.set_active_timeline(*timeline.name());

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StateTimelineView::identifier(),
        ))
    });

    // The state entity is already shown, so there is nothing to add.
    assert!(is_visualized(&test_context, view_id, &state_entity));
    let feedback = drop_entities(
        &test_context,
        view_id,
        &[state_entity],
        /* released */ false,
    );
    assert_eq!(
        feedback,
        DragAndDropFeedback::Reject(Some("Already in this view"))
    );

    // The scalar entity is matched by the wildcard too, but the view has no use for it: the
    // class-specific reason takes precedence over "already in this view".
    let feedback = drop_entities(
        &test_context,
        view_id,
        &[scalar_entity],
        /* released */ false,
    );
    assert_eq!(feedback, DragAndDropFeedback::Reject(Some(NO_STATE_DATA)));
}
