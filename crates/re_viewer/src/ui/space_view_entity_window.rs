use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityTree, InstancePath};

use crate::misc::{space_info::SpaceInfoCollection, UnreachableTransform, ViewerContext};

use super::{
    view_category::{categorize_entity_path, ViewCategory},
    SpaceView, SpaceViewId,
};

/// Window for adding/removing entities from a space view.
pub struct SpaceViewEntityWindow {
    pub space_view_id: SpaceViewId,
}

impl SpaceViewEntityWindow {
    pub fn ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view: &mut SpaceView,
    ) -> bool {
        let visuals_before = ui.visuals().clone();
        ui.visuals_mut().window_fill = visuals_before.widgets.inactive.bg_fill;
        ui.visuals_mut().window_stroke = visuals_before.widgets.active.fg_stroke;

        let mut open = true;
        egui::Window::new(format!("Add/remove entities to \"{}\"", space_view.name))
            //.pivot(egui::Align2::CENTER_CENTER)
            //.default_pos(ui.ctx().screen_rect().center())
            .collapsible(false)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                self.add_entities_ui(ctx, ui, space_view);
            });

        *ui.visuals_mut() = visuals_before;

        open
    }

    #[allow(clippy::unused_self)]
    fn add_entities_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view: &mut SpaceView,
    ) {
        let spaces_info = SpaceInfoCollection::new(&ctx.log_db.entity_db);
        if let Some(tree) = ctx.log_db.entity_db.tree.subtree(&space_view.root_path) {
            self.add_entities_tree_ui(
                ctx,
                ui,
                &spaces_info,
                &tree.path.to_string(),
                tree,
                space_view,
            );
        }
    }

    #[allow(clippy::unused_self)]
    fn add_entities_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        name: &str,
        tree: &EntityTree,
        space_view: &mut SpaceView,
    ) {
        if tree.is_leaf() {
            self.add_entities_line_ui(
                ctx,
                ui,
                spaces_info,
                &format!("🔹 {name}"),
                &tree.path,
                space_view,
            );
        } else {
            let default_open =
                space_view.space_path.is_descendant_of(&tree.path) || tree.children.len() <= 3;
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(name),
                default_open,
            )
            .show_header(ui, |ui| {
                self.add_entities_line_ui(ctx, ui, spaces_info, name, &tree.path, space_view);
            })
            .body(|ui| {
                for (path_comp, child_tree) in &tree.children {
                    self.add_entities_tree_ui(
                        ctx,
                        ui,
                        spaces_info,
                        &path_comp.to_string(),
                        child_tree,
                        space_view,
                    );
                }
            });
        };
    }

    #[allow(clippy::unused_self)]
    fn add_entities_line_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        name: &str,
        entity_path: &EntityPath,
        space_view: &mut SpaceView,
    ) {
        ui.horizontal(|ui| {
            let space_view_id = if space_view.data_blueprint.contains_entity(entity_path) {
                Some(space_view.id)
            } else {
                None
            };

            let widget_text = if entity_path == &space_view.space_path {
                egui::RichText::new(name).strong()
            } else {
                egui::RichText::new(name)
            };
            ctx.instance_path_button_to(ui, space_view_id, &InstancePath::entity_splat(entity_path.clone()), widget_text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let entity_tree = &ctx.log_db.entity_db.tree;

                if space_view.data_blueprint.contains_entity(entity_path) {
                    if ui
                    .button("➖")
                    .on_hover_text("Remove this path from the Space View")
                    .clicked()
                    {
                        // Remove all entities at and under this path
                        entity_tree.subtree(entity_path)
                        .unwrap()
                        .visit_children_recursively(&mut |path: &EntityPath| {
                            space_view.data_blueprint.remove_entity(path);
                        });
                    }
                } else {
                    let entity_category = categorize_entity_path(Timeline::log_time(), ctx.log_db, entity_path);
                    let cannot_add_reason = if entity_category.contains(space_view.category) {
                        spaces_info.is_reachable_by_transform(entity_path, &space_view.space_path).map_err
                        (|reason| match reason {
                            // Should never happen
                            UnreachableTransform::Unconnected =>
                                 "No entity path connection from this space view.",
                            UnreachableTransform::NestedPinholeCameras =>
                                "Can't display entities under nested pinhole cameras.",
                            UnreachableTransform::UnknownTransform =>
                                "Can't display entities that are connected via an unknown transform to this space.",
                            UnreachableTransform::InversePinholeCameraWithoutResolution =>
                                "Can't display entities that would require inverting a pinhole camera without a specified resolution.",
                        }).err()
                    } else if entity_category.is_empty() {
                        Some("Entity does not have any components")
                    } else {
                        Some("Entity category can't be displayed by this type of spatial view")
                    };

                    let response = ui.add_enabled_ui(cannot_add_reason.is_none(), |ui| {
                        let response = ui.button("➕").on_hover_text("Add this entity to the Space View");
                        if response.clicked() {
                            // Insert the entity it space_view and all its children as far as they haven't been added yet
                            let mut entities = Vec::new();
                            entity_tree
                                .subtree(entity_path)
                                .unwrap()
                                .visit_children_recursively(&mut |path: &EntityPath| {
                                    if has_visualization_for_category(ctx, space_view.category, path)
                                        && !space_view.data_blueprint.contains_entity(path)
                                        && spaces_info.is_reachable_by_transform(path, &space_view.space_path).is_ok()
                                    {
                                        entities.push(path.clone());
                                    }
                                });
                            space_view.data_blueprint.insert_entities_according_to_hierarchy(entities.iter(), &space_view.space_path);
                            space_view.entities_determined_by_user = true;
                        }
                    }).response;

                    if let Some(cannot_add_reason) = cannot_add_reason {
                        response.on_hover_text(cannot_add_reason);
                    }
                }
            });
        });
    }
}

fn has_visualization_for_category(
    ctx: &ViewerContext<'_>,
    category: ViewCategory,
    entity_path: &EntityPath,
) -> bool {
    let log_db = &ctx.log_db;
    categorize_entity_path(Timeline::log_time(), log_db, entity_path).contains(category)
}
