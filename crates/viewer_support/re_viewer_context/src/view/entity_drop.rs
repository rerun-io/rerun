use re_log_types::{EntityPath, EntityPathFilter};
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes::ViewContents;
use re_sdk_types::blueprint::components::QueryExpression;

use crate::{DragAndDropFeedback, ViewContext, entity_path_for_view_property};

/// Handle entities being dragged over a view.
///
/// This is what [`crate::ViewClass::handle_drop`] does for entities by default.
///
/// A dragged entity is added with an including-subtree rule, so it is worth dropping as soon
/// as one entity of its subtree, possibly itself, is:
/// - visualizable by the class of the view,
/// - not part of the view yet,
/// - and not rejected by `reject_reason`.
///
/// All three must hold for the *same* entity: a subtree whose only additions would be
/// entities the view can't show has nothing to contribute.
///
/// Design decisions:
/// - We accept the drop as soon as one of the dragged entities is worth dropping.
///     - When the drop happens, of all dragged entities, we only add those.
/// - If nothing is left to accept, we show the most specific reason we have.
///
/// The blueprint only changes when `released` is true.
pub fn default_view_entity_drop(
    ctx: &ViewContext<'_>,
    entities: &[EntityPath],
    released: bool,
    reject_reason: impl Fn(&EntityPath) -> Option<&'static str>,
) -> DragAndDropFeedback {
    let recording_engine = ctx.recording_engine();
    let entity_tree = recording_engine.store().entity_tree();

    let mut acceptable_entities = Vec::new();

    // Why we had to turn an entity down, in decreasing order of specificity. Only shown if
    // nothing is acceptable in the end.
    let mut rejection = None;
    let mut any_already_in_view = false;

    for entity in entities {
        let Some(subtree) = entity_tree.subtree(entity) else {
            continue;
        };

        let mut is_acceptable = false;
        subtree.visit_children_recursively(|entity_path| {
            // Entities no visualizer of this class can show are silently ignored.
            let is_visualizable = ctx
                .viewer_ctx
                .iter_visualizable_entities_for_view_class(ctx.view_class_identifier)
                .any(|(_, entities)| entities.contains_key(entity_path));
            if !is_visualizable {
                return;
            }

            if let Some(reason) = reject_reason(entity_path) {
                rejection.get_or_insert(reason);
            } else if ctx.query_result.result_for_entity(entity_path).is_some() {
                any_already_in_view = true;
            } else {
                is_acceptable = true;
            }
        });

        if is_acceptable {
            acceptable_entities.push(entity);
        }
    }

    if acceptable_entities.is_empty() {
        let reason = rejection.or_else(|| any_already_in_view.then_some("Already in this view"));
        return DragAndDropFeedback::Reject(reason);
    }

    if released {
        // The stored expressions are unresolved, so `$origin` rules stay intact.
        let contents_path = entity_path_for_view_property(
            ctx.view_id,
            ctx.blueprint_db().storage_engine().store().entity_tree(),
            ViewContents::name(),
        );
        let query_descr = ViewContents::descriptor_query();
        let expressions = ctx
            .blueprint_db()
            .latest_at(
                ctx.blueprint_query(),
                &contents_path,
                [query_descr.component],
            )
            .component_batch::<QueryExpression>(query_descr.component)
            .unwrap_or_default();

        let mut filter =
            EntityPathFilter::from_query_expressions(expressions.iter().map(|e| e.0.as_str()));
        for entity in acceptable_entities {
            filter.add_entity_subtree(entity);
        }

        ctx.save_blueprint_component(
            contents_path,
            &query_descr,
            &filter
                .iter_expressions()
                .map(|e| QueryExpression(e.into()))
                .collect::<Vec<_>>(),
        );
    }

    DragAndDropFeedback::Accept
}
