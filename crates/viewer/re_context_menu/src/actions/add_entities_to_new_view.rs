use egui::{Response, Ui};
use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_log_types::{EntityPath, EntityPathFilter, EntityPathHash, EntityPathRule, RuleEffect};
use re_sdk_types::ViewClassIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{
    Item, RecommendedView, SystemCommand, SystemCommandSender as _, ViewClass,
    ViewSystemIdentifier, ViewerContext, VisualizableEntities, VisualizableReason,
};
use re_viewport_blueprint::ViewBlueprint;
use smallvec::SmallVec;

use crate::{ContextMenuAction, ContextMenuContext};

/// Create a new view containing the selected entities.
///
/// The view is created next to the clicked item's parent view (if a data result was clicked).
pub(crate) struct AddEntitiesToNewViewAction;

impl ContextMenuAction for AddEntitiesToNewViewAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::DataResult(_) | Item::InstancePath(_))
    }

    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut Ui) -> Response {
        let view_class_registry = ctx.viewer_context.view_class_registry();

        let recommended_view_classes = recommended_views_for_selection(ctx);
        let other_view_classes: IntSet<_> = view_class_registry
            .iter_registry()
            .map(|entry| entry.identifier)
            .collect::<IntSet<ViewClassIdentifier>>()
            .difference(&recommended_view_classes)
            .copied()
            .collect();

        ui.menu_button("Add to new view", |ui| {
            let buttons_for_view_classes =
                |ui: &mut egui::Ui, view_classes: &IntSet<ViewClassIdentifier>| {
                    for (identifier, class) in view_classes
                        .iter()
                        .map(|identifier| {
                            (
                                identifier,
                                view_class_registry.get_class_or_log_error(*identifier),
                            )
                        })
                        .sorted_by_key(|(_, class)| {
                            (
                                class.recommendation_order(),
                                class.display_name().to_owned(),
                            )
                        })
                    {
                        let btn = class
                            .icon()
                            .as_button_with_label(ui.tokens(), class.display_name());
                        if ui.add(btn).clicked() {
                            create_view_for_selected_entities(ctx, *identifier);
                            ui.close();
                        }
                    }
                };
            if recommended_view_classes.is_empty() {
                ui.label("None");
            } else {
                buttons_for_view_classes(ui, &recommended_view_classes);
            }

            if !other_view_classes.is_empty() {
                ui.separator();
                ui.menu_button("Other views", |ui| {
                    buttons_for_view_classes(ui, &other_view_classes);
                });
            }
        })
        .response
    }
}

/// Builds a list of views that are a good fit for the provided selection.
fn recommended_views_for_selection(ctx: &ContextMenuContext<'_>) -> IntSet<ViewClassIdentifier> {
    re_tracing::profile_function!();

    let entities_of_interest = ctx
        .selection
        .iter()
        .filter_map(|(item, _)| item.entity_path().cloned())
        .collect::<IntSet<_>>();

    let mut output: IntSet<ViewClassIdentifier> = IntSet::default();

    let view_class_registry = ctx.viewer_context.view_class_registry();

    for entry in view_class_registry.iter_registry() {
        let class = view_class_registry.get_class_or_log_error(entry.identifier);
        let visualizers: SmallVec<[(ViewSystemIdentifier, &VisualizableEntities); 32]> = ctx
            .viewer_context
            .iter_visualizable_entities_for_view_class(entry.identifier)
            .collect();

        // We consider a view class to be recommended if all selected entities are "shown" by it.
        // By "shown" we mean that either the entity itself, or any of its sub-entities, would be
        // displayed in a new view of this class.
        let covered = entities_of_interest.iter().all(|candidate_entity| {
            would_show_subtree(ctx.viewer_context, class, &visualizers, candidate_entity)
        });

        if covered {
            output.insert(entry.identifier);
        }
    }

    output
}

/// Would a newly created view of `class` show `candidate_entity` or any of its sub-entities?
fn would_show_subtree(
    viewer_ctx: &ViewerContext<'_>,
    class: &dyn ViewClass,
    visualizers: &[(ViewSystemIdentifier, &VisualizableEntities)],
    candidate_entity: &EntityPath,
) -> bool {
    // Check the selected entity itself first: it's by far the most common match, and it saves us
    // from walking the (potentially very large) list of visualizable entities below.
    if would_show_entity(viewer_ctx, class, visualizers, candidate_entity) {
        return true;
    }

    // This can grow to the size of the subtree, so a linear-scan container won't do.
    let mut already_checked: IntSet<EntityPathHash> =
        std::iter::once(candidate_entity.hash()).collect();

    for (_visualizer, visualizable_entities) in visualizers {
        #[expect(clippy::iter_over_hash_type)] // We stop at the first match, order doesn't matter.
        for entity_path in visualizable_entities.keys() {
            if entity_path.starts_with(candidate_entity)
                && already_checked.insert(entity_path.hash())
                && would_show_entity(viewer_ctx, class, visualizers, entity_path)
            {
                return true;
            }
        }
    }

    false
}

/// Would a newly created view of `class` show `entity_path` right away?
fn would_show_entity(
    viewer_ctx: &ViewerContext<'_>,
    class: &dyn ViewClass,
    visualizers: &[(ViewSystemIdentifier, &VisualizableEntities)],
    entity_path: &EntityPath,
) -> bool {
    // This is the same per-entity input that view contents resolution feeds into
    // `ViewClass::recommended_visualizers_for_entity`, see `ViewContents::build_data_result_tree`.
    let visualizers_with_reason: SmallVec<[(ViewSystemIdentifier, &VisualizableReason); 8]> =
        visualizers
            .iter()
            .filter_map(|(visualizer, visualizable_entities)| {
                visualizable_entities
                    .get(entity_path)
                    .map(|reason| (*visualizer, reason))
            })
            .collect();

    !class
        .recommended_visualizers_for_entity(
            entity_path,
            &visualizers_with_reason,
            viewer_ctx.indicated_entities_per_visualizer,
        )
        .into_auto_spawned()
        .is_empty()
}

/// Creates a view of the given class, with root set as origin, and a filter set to include all
/// selected entities. Then, the selection is set to the new view.
fn create_view_for_selected_entities(
    ctx: &ContextMenuContext<'_>,
    identifier: ViewClassIdentifier,
) {
    let entities_of_interest = ctx
        .selection
        .iter()
        .filter_map(|(item, _)| item.entity_path().cloned())
        .collect::<IntSet<_>>();

    let origin = ctx
        .viewer_context
        .view_class_registry()
        .get_class_or_log_error(identifier)
        .recommended_origin_for_entities(&entities_of_interest, ctx.viewer_context.recording())
        .unwrap_or_else(EntityPath::root);

    let mut query_filter = EntityPathFilter::default();

    let target_container_id = ctx
        .clicked_item_enclosing_container_id_and_position()
        .map(|(id, _)| id);

    // Note that these entity paths will always be absolute, rather than
    // relative to the origin. This makes sense since if you create a view and
    // then change the origin you likely wanted those entities to still be there.

    #[expect(clippy::iter_over_hash_type)] // Order of rule insertion does not matter here
    for path in entities_of_interest {
        query_filter.insert_rule(
            RuleEffect::Include,
            EntityPathRule::including_entity_subtree(&path),
        );
    }
    let recommended = RecommendedView {
        origin,
        query_filter,
    };

    let view = ViewBlueprint::new(identifier, recommended);
    let view_id = view.id;
    ctx.viewport_blueprint
        .add_views(std::iter::once(view), target_container_id, None);
    ctx.viewer_context
        .command_sender()
        .send_system(SystemCommand::set_selection(Item::View(view_id)));
    ctx.viewport_blueprint
        .mark_user_interaction(ctx.viewer_context);
}
