use itertools::Itertools;
use nohash_hasher::IntMap;

use re_data_ui::item_ui;
use re_entity_db::{EntityPath, EntityTree, InstancePath};
use re_log_types::{EntityPathFilter, EntityPathRule};
use re_viewer_context::{DataQueryResult, SpaceViewId, ViewerContext};

use crate::{
    determine_visualizable_entities, space_info::SpaceInfoCollection,
    space_view::SpaceViewBlueprint, ViewportBlueprint,
};

/// Window for adding/removing entities from a space view.
///
/// Delegates to [`re_ui::modal::ModalHandler`]
#[derive(Default)]
pub struct SpaceViewEntityPicker {
    space_view_id: Option<SpaceViewId>,
    modal_handler: re_ui::modal::ModalHandler,
}

impl SpaceViewEntityPicker {
    pub fn open(&mut self, space_view_id: SpaceViewId) {
        self.space_view_id = Some(space_view_id);
        self.modal_handler.open();
    }

    #[allow(clippy::unused_self)]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
    ) {
        self.modal_handler.ui(
            ctx.re_ui,
            ui,
            || re_ui::modal::Modal::new("Add/remove Entities").default_height(640.0),
            |_, ui, open| {
                let Some(space_view_id) = &self.space_view_id else {
                    *open = false;
                    return;
                };

                let Some(space_view) = viewport_blueprint.space_views.get(space_view_id) else {
                    *open = false;
                    return;
                };

                egui::ScrollArea::vertical().show(ui, |ui| {
                    add_entities_ui(ctx, ui, space_view);
                });
            },
        );
    }
}

fn add_entities_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, space_view: &SpaceViewBlueprint) {
    re_tracing::profile_function!();

    let spaces_info = SpaceInfoCollection::new(ctx.entity_db);
    let tree = &ctx.entity_db.tree();
    // TODO(jleibs): Avoid clone
    let query_result = ctx.lookup_query_result(space_view.query_id()).clone();
    let entity_path_filter = space_view.entity_path_filter();
    let entities_add_info =
        create_entity_add_info(ctx, tree, space_view, &query_result, &spaces_info);

    add_entities_tree_ui(
        ctx,
        ui,
        &tree.path.to_string(),
        tree,
        space_view,
        &query_result,
        &entity_path_filter,
        &entities_add_info,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_entities_tree_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    name: &str,
    tree: &EntityTree,
    space_view: &SpaceViewBlueprint,
    query_result: &DataQueryResult,
    entity_path_filter: &EntityPathFilter,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    if tree.is_leaf() {
        add_entities_line_ui(
            ctx,
            ui,
            &format!("🔹 {name}"),
            tree,
            space_view,
            query_result,
            entity_path_filter,
            entities_add_info,
        );
    } else {
        let level = tree.path.len();
        let default_open = space_view.space_origin.is_descendant_of(&tree.path)
            || tree.children.len() <= 3
            || level < 2;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.id().with(name),
            default_open,
        )
        .show_header(ui, |ui| {
            add_entities_line_ui(
                ctx,
                ui,
                name,
                tree,
                space_view,
                query_result,
                entity_path_filter,
                entities_add_info,
            );
        })
        .body(|ui| {
            for (path_comp, child_tree) in tree.children.iter().sorted_by_key(|(_, child_tree)| {
                // Put descendants of the space path always first
                let put_first = child_tree.path.starts_with(&space_view.space_origin);
                !put_first
            }) {
                add_entities_tree_ui(
                    ctx,
                    ui,
                    &path_comp.ui_string(),
                    child_tree,
                    space_view,
                    query_result,
                    entity_path_filter,
                    entities_add_info,
                );
            }
        });
    };
}

#[allow(clippy::too_many_arguments)]
fn add_entities_line_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    name: &str,
    entity_tree: &EntityTree,
    space_view: &SpaceViewBlueprint,
    query_result: &DataQueryResult,
    entity_path_filter: &EntityPathFilter,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    re_tracing::profile_function!();

    let query = ctx.current_query();
    let store = ctx.entity_db.store();

    ui.horizontal(|ui| {
        let entity_path = &entity_tree.path;

        let add_info = entities_add_info.get(entity_path).unwrap();

        let is_explicitly_excluded = entity_path_filter.is_explicitly_excluded(entity_path);
        let is_explicitly_included = entity_path_filter.is_explicitly_included(entity_path);
        let is_included = entity_path_filter.is_included(entity_path);

        ui.add_enabled_ui(add_info.can_add_self_or_descendant.is_compatible(), |ui| {
            let widget_text = if is_explicitly_excluded {
                // TODO(jleibs): Better design-language for excluded.
                egui::RichText::new(name).italics()
            } else if entity_path == &space_view.space_origin {
                egui::RichText::new(name).strong()
            } else {
                egui::RichText::new(name)
            };
            let response = item_ui::instance_path_button_to(
                ctx,
                &query,
                store,
                ui,
                Some(space_view.id),
                &InstancePath::entity_splat(entity_path.clone()),
                widget_text,
            );
            if query_result.contains_entity(entity_path) {
                response.highlight();
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if entity_path_filter.contains_rule_for_exactly(entity_path) {
                // Reset-button
                // Shows when an entity is explicitly excluded or included
                let response = ctx.re_ui.small_icon_button(ui, &re_ui::icons::RESET);

                if response.clicked() {
                    space_view.remove_filter_rule_for(ctx, &entity_tree.path);
                }

                if is_explicitly_excluded {
                    response.on_hover_text("Stop excluding this EntityPath.");
                } else if is_explicitly_included {
                    response.on_hover_text("Stop including this EntityPath.");
                }
            } else if is_included {
                // Remove-button
                // Shows when an entity is already included (but not explicitly)
                let response = ctx.re_ui.small_icon_button(ui, &re_ui::icons::REMOVE);

                if response.clicked() {
                    space_view.add_entity_exclusion(
                        ctx,
                        EntityPathRule::including_subtree(entity_tree.path.clone()),
                    );
                }

                response.on_hover_text(
                    "Exclude this Entity and all its descendants from the Space View",
                );
            } else {
                // Add-button:
                // Shows when an entity is not included
                // Only enabled if the entity is compatible.
                let enabled = add_info.can_add_self_or_descendant.is_compatible();

                ui.add_enabled_ui(enabled, |ui| {
                    let response = ctx.re_ui.small_icon_button(ui, &re_ui::icons::ADD);

                    if response.clicked() {
                        space_view.add_entity_inclusion(
                            ctx,
                            EntityPathRule::including_subtree(entity_tree.path.clone()),
                        );
                    }

                    if enabled {
                        if add_info.can_add.is_compatible_and_missing() {
                            response.on_hover_text(
                                "Include this Entity and all its descendants in the Space View",
                            );
                        } else {
                            response
                                .on_hover_text("Add descendants of this Entity to the Space View");
                        }
                    } else if let CanAddToSpaceView::No { reason } = &add_info.can_add {
                        response.on_disabled_hover_text(reason);
                    }
                });
            }
        });
    });
}

/// Describes if an entity path can be added to a space view.
#[derive(Clone, PartialEq, Eq)]
enum CanAddToSpaceView {
    Compatible { already_added: bool },
    No { reason: String },
}

impl Default for CanAddToSpaceView {
    fn default() -> Self {
        Self::Compatible {
            already_added: false,
        }
    }
}

impl CanAddToSpaceView {
    /// Can be generally added but space view might already have this element.
    pub fn is_compatible(&self) -> bool {
        match self {
            CanAddToSpaceView::Compatible { .. } => true,
            CanAddToSpaceView::No { .. } => false,
        }
    }

    /// Can be added and spaceview doesn't have it already.
    pub fn is_compatible_and_missing(&self) -> bool {
        self == &CanAddToSpaceView::Compatible {
            already_added: false,
        }
    }

    pub fn join(&self, other: &CanAddToSpaceView) -> CanAddToSpaceView {
        match self {
            CanAddToSpaceView::Compatible { already_added } => {
                let already_added = if let CanAddToSpaceView::Compatible {
                    already_added: already_added_other,
                } = other
                {
                    *already_added && *already_added_other
                } else {
                    *already_added
                };
                CanAddToSpaceView::Compatible { already_added }
            }
            CanAddToSpaceView::No { .. } => other.clone(),
        }
    }
}

#[derive(Default)]
#[allow(dead_code)]
struct EntityAddInfo {
    can_add: CanAddToSpaceView,
    can_add_self_or_descendant: CanAddToSpaceView,
}

fn create_entity_add_info(
    ctx: &ViewerContext<'_>,
    tree: &EntityTree,
    space_view: &SpaceViewBlueprint,
    query_result: &DataQueryResult,
    spaces_info: &SpaceInfoCollection,
) -> IntMap<EntityPath, EntityAddInfo> {
    let mut meta_data: IntMap<EntityPath, EntityAddInfo> = IntMap::default();

    // TODO(andreas): This should be state that is already available because it's part of the space view's state.
    let class = space_view.class(ctx.space_view_class_registry);
    let visualizable_entities = determine_visualizable_entities(
        ctx.applicable_entities_per_visualizer,
        ctx.entity_db,
        &ctx.space_view_class_registry
            .new_visualizer_collection(class.identifier()),
        class,
        &space_view.space_origin,
    );

    tree.visit_children_recursively(&mut |entity_path, _| {
        let can_add: CanAddToSpaceView =
            if visualizable_entities.iter().any(|(_, entities)| entities.contains(entity_path)) {
                // TODO(andreas): (topological) reachability should be part of visualizability.
                //                Yes, this means that once an entity is no longer visualizable (due to pinhole etc.) it stays this way.
                match spaces_info.is_reachable_by_transform(entity_path, &space_view.space_origin) {
                    Ok(()) => CanAddToSpaceView::Compatible {
                        already_added: query_result.contains_any(entity_path),
                    },
                    Err(reason) => CanAddToSpaceView::No {
                        reason: reason.to_string(),
                    },
                }
            } else {
                CanAddToSpaceView::No {
                    reason: format!(
                        "Entity can't be displayed by this class of Space View ({}), since it doesn't match any archetype that the Space View can process.",
                        space_view.class_identifier()
                    ),
                }
            };

        if can_add.is_compatible() {
            // Mark parents aware that there is some descendant that is compatible
            let mut path = entity_path.clone();
            while let Some(parent) = path.parent() {
                let data = meta_data.entry(parent.clone()).or_default();
                data.can_add_self_or_descendant = data.can_add_self_or_descendant.join(&can_add);
                path = parent;
            }
        }

        let can_add_self_or_descendant = can_add.clone();
        meta_data.insert(
            entity_path.clone(),
            EntityAddInfo {
                can_add,
                can_add_self_or_descendant,
            },
        );
    });

    meta_data
}
