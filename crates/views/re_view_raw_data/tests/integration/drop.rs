//! Tests for dropping entities and components onto a raw data view.

use re_chunk_store::RowId;
use re_log_types::{
    ComponentPath, EntityPath, EntityPathFilter, ResolvedEntityPathFilter, TimePoint, Timeline,
};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::RawDataRows;
use re_sdk_types::blueprint::components::ComponentIdentifier as BlueprintComponentIdentifier;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_raw_data::RawDataView;
use re_viewer_context::{
    DragAndDropFeedback, DragAndDropPayload, RecommendedView, ViewClass as _, ViewId,
};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// The entity the view shows before any drop.
const OLD_ENTITY: &str = "old";

/// A view showing [`OLD_ENTITY`], with `rows` as its component checklist.
///
/// [`OLD_ENTITY`] has data, so the view displays it.
fn setup_view(rows: Option<&[&str]>) -> (TestContext, ViewId) {
    let mut test_context = TestContext::new_with_view_class::<RawDataView>();
    let timeline = Timeline::new_sequence("frame");
    test_context.log_entity(OLD_ENTITY, |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(timeline, 1)]),
            &TextLog::new("message"),
        )
    });
    test_context.set_active_timeline(*timeline.name());

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new(
            RawDataView::identifier(),
            RecommendedView::new_single_entity(EntityPath::from(OLD_ENTITY)),
        );
        if let Some(rows) = rows {
            ViewProperty::from_archetype_for_view::<RawDataRows>(ctx, view.id)
                .save_blueprint_component(
                    ctx,
                    &RawDataRows::descriptor_data_rows(),
                    &blueprint_rows(rows),
                );
        }
        blueprint.add_view_at_root(view)
    });
    (test_context, view_id)
}

fn blueprint_rows(rows: &[&str]) -> Vec<BlueprintComponentIdentifier> {
    rows.iter()
        .map(|row| BlueprintComponentIdentifier::from(*row))
        .collect()
}

fn entities(paths: &[&str]) -> DragAndDropPayload {
    DragAndDropPayload::Entities {
        entities: paths
            .iter()
            .map(|path| EntityPath::from(*path))
            .collect::<Vec<_>>()
            .try_into()
            .expect("entity payload must not be empty"),
    }
}

fn components(paths: &[(&str, &'static str)]) -> DragAndDropPayload {
    DragAndDropPayload::Components {
        component_paths: paths
            .iter()
            .map(|(entity, component)| {
                ComponentPath::new(EntityPath::from(*entity), (*component).into())
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("component payload must not be empty"),
    }
}

/// Drop `payload` onto `view_id` and return the feedback the viewport would show.
fn drop_payload(
    test_context: &TestContext,
    view_id: ViewId,
    payload: &DragAndDropPayload,
) -> DragAndDropFeedback {
    let feedback = test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        RawDataView.handle_drop(
            &ViewBlueprint::try_from_db(view_id, ctx.blueprint_db(), ctx.blueprint_query)
                .expect("view should exist")
                .bundle_context_with_state(ctx, RawDataView.new_state().as_ref()),
            payload,
            true,
        )
    });
    test_context.handle_system_commands(&egui::Context::default());
    feedback
}

fn entity_filter(test_context: &TestContext, view_id: ViewId) -> ResolvedEntityPathFilter {
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let view = ViewBlueprint::try_from_db(view_id, ctx.blueprint_db(), ctx.blueprint_query)
            .expect("view should exist");
        view.contents.entity_path_filter().clone()
    })
}

fn rows(test_context: &TestContext, view_id: ViewId) -> Option<Vec<BlueprintComponentIdentifier>> {
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        ViewProperty::from_archetype_for_view::<RawDataRows>(ctx, view_id)
            .component_array::<BlueprintComponentIdentifier>(
                RawDataRows::descriptor_data_rows().component,
            )
            .expect("rows should deserialize")
    })
}

fn single_entity_filter(path: &str) -> ResolvedEntityPathFilter {
    EntityPathFilter::single_entity_filter(&EntityPath::from(path)).resolve_without_substitutions()
}

#[test]
fn entity_drop_shows_only_that_entity_with_all_components() {
    let (test_context, view_id) = setup_view(Some(&["old:value"]));

    let feedback = drop_payload(&test_context, view_id, &entities(&["new"]));

    assert_eq!(feedback, DragAndDropFeedback::Accept);
    assert_eq!(
        entity_filter(&test_context, view_id),
        single_entity_filter("new"),
        "the filter should match the dropped entity, but not its children"
    );
    assert_eq!(
        rows(&test_context, view_id),
        None,
        "dropping a new entity should show all its components"
    );
}

#[test]
fn entity_drop_on_displayed_entity_shows_all_components() {
    let (test_context, view_id) = setup_view(Some(&["old:value"]));

    let feedback = drop_payload(&test_context, view_id, &entities(&[OLD_ENTITY]));

    assert_eq!(feedback, DragAndDropFeedback::Accept);
    assert_eq!(
        rows(&test_context, view_id),
        None,
        "dropping the displayed entity should show all its components"
    );
}

#[test]
fn several_entities_are_rejected() {
    let (test_context, view_id) = setup_view(None);

    let feedback = drop_payload(&test_context, view_id, &entities(&["new", "other"]));

    assert_eq!(
        feedback,
        DragAndDropFeedback::Reject(Some("Drop exactly one entity"))
    );
    assert_eq!(
        entity_filter(&test_context, view_id),
        single_entity_filter(OLD_ENTITY)
    );
}

#[test]
fn component_drop_shows_that_entity_with_only_those_components() {
    let (test_context, view_id) = setup_view(Some(&["old:value"]));

    let feedback = drop_payload(
        &test_context,
        view_id,
        &components(&[
            ("new", "new:value"),
            ("new", "new:other"),
            ("new", "new:other"),
        ]),
    );

    assert_eq!(feedback, DragAndDropFeedback::Accept);
    assert_eq!(
        entity_filter(&test_context, view_id),
        single_entity_filter("new")
    );
    assert_eq!(
        rows(&test_context, view_id),
        Some(blueprint_rows(&["new:value", "new:other"])),
        "the rows should be the dropped components, without duplicates"
    );
}

#[test]
fn component_drop_on_displayed_entity_adds_rows() {
    let (test_context, view_id) = setup_view(Some(&["old:value"]));

    let feedback = drop_payload(
        &test_context,
        view_id,
        &components(&[(OLD_ENTITY, "old:other")]),
    );

    assert_eq!(feedback, DragAndDropFeedback::Accept);
    assert_eq!(
        rows(&test_context, view_id),
        Some(blueprint_rows(&["old:value", "old:other"]))
    );
}

#[test]
fn components_of_several_entities_are_rejected() {
    let (test_context, view_id) = setup_view(Some(&["old:value"]));

    let feedback = drop_payload(
        &test_context,
        view_id,
        &components(&[("new", "new:value"), ("other", "other:value")]),
    );

    assert_eq!(
        feedback,
        DragAndDropFeedback::Reject(Some("Drop components from a single entity"))
    );
    assert_eq!(
        entity_filter(&test_context, view_id),
        single_entity_filter(OLD_ENTITY)
    );
    assert_eq!(
        rows(&test_context, view_id),
        Some(blueprint_rows(&["old:value"]))
    );
}
