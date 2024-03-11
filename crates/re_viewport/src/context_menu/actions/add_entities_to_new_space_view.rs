use egui::{Response, Ui};
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_log_types::{EntityPath, EntityPathFilter, EntityPathRule, RuleEffect};
use re_space_view::{determine_visualizable_entities, SpaceViewBlueprint};
use re_viewer_context::{Item, SpaceViewClassIdentifier};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};

/// Create a new space view containing the selected entities.
///
/// The space view is created next to the clicked item's parent view (if a data result was clicked).
pub(crate) struct AddEntitiesToNewSpaceViewAction;

impl ContextMenuAction for AddEntitiesToNewSpaceViewAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::DataResult(_, _) | Item::InstancePath(_))
    }

    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut Ui) -> Response {
        let space_view_class_registry = ctx.viewer_context.space_view_class_registry;

        let recommended_space_view_classes = recommended_space_views_for_selection(ctx);
        let other_space_view_classes: IntSet<_> = space_view_class_registry
            .iter_registry()
            .map(|entry| entry.class.identifier())
            .collect::<IntSet<SpaceViewClassIdentifier>>()
            .difference(&recommended_space_view_classes)
            .cloned()
            .collect();

        ui.menu_button("Add to new Space View", |ui| {
            let buttons_for_space_view_classes =
                |ui: &mut egui::Ui, space_view_classes: &IntSet<SpaceViewClassIdentifier>| {
                    for (identifier, display_name) in space_view_classes
                        .iter()
                        .map(|identifier| {
                            (
                                identifier,
                                space_view_class_registry
                                    .get_class_or_log_error(identifier)
                                    .display_name(),
                            )
                        })
                        .sorted_by_key(|(_, display_name)| display_name.to_owned())
                    {
                        if ui.button(display_name).clicked() {
                            create_space_view_for_selected_entities(ctx, *identifier);
                            ui.close_menu();
                        }
                    }
                };

            ui.label(egui::WidgetText::from("Recommended:").italics());
            if recommended_space_view_classes.is_empty() {
                ui.label("None");
            } else {
                buttons_for_space_view_classes(ui, &recommended_space_view_classes);
            }

            if !other_space_view_classes.is_empty() {
                ui.label(egui::WidgetText::from("Others:").italics());
                buttons_for_space_view_classes(ui, &other_space_view_classes);
            }
        })
        .response
    }
}

/// Builds a list of compatible space views for the provided selection.
fn recommended_space_views_for_selection(
    ctx: &ContextMenuContext<'_>,
) -> IntSet<SpaceViewClassIdentifier> {
    re_tracing::profile_function!();

    let entities_of_interest = ctx
        .selection
        .iter()
        .filter_map(|(item, _)| item.entity_path().cloned())
        .collect::<IntSet<_>>();

    let mut output: IntSet<SpaceViewClassIdentifier> = IntSet::default();

    let space_view_class_registry = ctx.viewer_context.space_view_class_registry;
    let entity_db = ctx.viewer_context.entity_db;
    let applicable_entities_per_visualizer =
        space_view_class_registry.applicable_entities_for_visualizer_systems(entity_db.store_id());

    for entry in space_view_class_registry.iter_registry() {
        let Some(suggested_root) = entry
            .class
            .recommended_root_for_entities(&entities_of_interest, entity_db)
        else {
            continue;
        };

        let visualizable_entities = determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &space_view_class_registry.new_visualizer_collection(entry.class.identifier()),
            &*entry.class,
            &suggested_root,
        );

        // We consider a space view class to be recommended if all selected entities are
        // "visualizable" with it. By "visualizable" we mean that either the entity itself, or any
        // of its sub-entities, are visualizable.

        let covered = entities_of_interest.iter().all(|entity| {
            visualizable_entities.0.iter().any(|(_, entities)| {
                entities
                    .0
                    .iter()
                    .any(|visualizable_entity| visualizable_entity.starts_with(entity))
            })
        });

        if covered {
            output.insert(entry.class.identifier());
        }
    }

    output
}

/// Creates a space view of the given class, with root set as origin, and a filter set to include all
/// selected entities. Then, the selection is set to the new space view.
fn create_space_view_for_selected_entities(
    ctx: &ContextMenuContext<'_>,
    identifier: SpaceViewClassIdentifier,
) {
    let entities_of_interest = ctx
        .selection
        .iter()
        .filter_map(|(item, _)| item.entity_path().cloned())
        .collect::<IntSet<_>>();

    let origin = ctx
        .viewer_context
        .space_view_class_registry
        .get_class_or_log_error(&identifier)
        .recommended_root_for_entities(&entities_of_interest, ctx.viewer_context.entity_db)
        .unwrap_or_else(EntityPath::root);

    let mut filter = EntityPathFilter::default();

    for path in entities_of_interest {
        filter.add_rule(RuleEffect::Include, EntityPathRule::including_subtree(path));
    }

    let target_container_id = ctx
        .clicked_item_enclosing_container_id_and_position()
        .map(|(id, _)| id);

    let space_view = SpaceViewBlueprint::new(identifier, &origin, filter);

    let new_space_view = ctx.viewport_blueprint.add_space_views(
        std::iter::once(space_view),
        ctx.viewer_context,
        target_container_id,
        None,
    );
    if let Some(space_view_id) = new_space_view.first() {
        ctx.viewer_context
            .selection_state()
            .set_selection(Item::SpaceView(*space_view_id));
    }
    ctx.viewport_blueprint
        .mark_user_interaction(ctx.viewer_context);
}
