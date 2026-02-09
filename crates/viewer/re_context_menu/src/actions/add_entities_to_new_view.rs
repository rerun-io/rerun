use egui::{Response, Ui};
use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_log_types::{EntityPath, EntityPathFilter, EntityPathRule, RuleEffect};
use re_sdk_types::ViewClassIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{Item, RecommendedView, SystemCommand, SystemCommandSender as _};
use re_viewport_blueprint::ViewBlueprint;

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
        matches!(item, Item::DataResult(_, _) | Item::InstancePath(_))
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

/// Builds a list of compatible views for the provided selection.
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
        // We consider a view class to be recommended if all selected entities are
        // "visualizable" with it through a native type.
        // By "visualizable" we mean that either the entity itself, or any
        // of its sub-entities, are visualizable.

        let covered = entities_of_interest.iter().all(|candidate_entity| {
            ctx.viewer_context
                .iter_visualizable_entities_for_view_class(entry.identifier)
                .any(|(_visualizer, visualizable_entities)| {
                    visualizable_entities
                        .0
                        .iter()
                        .any(|(visualizable_entity, _reason)| {
                            // TODO(andreas): Do we want to consider certain reasons as more relevant than others?
                            // For example, should we consider native-semantics as more recommended?
                            visualizable_entity.starts_with(candidate_entity)
                        })
                })
        });

        if covered {
            output.insert(entry.identifier);
        }
    }

    output
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
