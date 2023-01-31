use itertools::Itertools;
use nohash_hasher::IntMap;
use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityTree, InstancePath};

use crate::misc::{space_info::SpaceInfoCollection, ViewerContext};

use super::{
    view_category::{categorize_entity_path, ViewCategory},
    SpaceView, SpaceViewId,
};

/// Window for adding/removing entities from a space view.
pub struct SpaceViewEntityPicker {
    pub space_view_id: SpaceViewId,
}

impl SpaceViewEntityPicker {
    #[allow(clippy::unused_self)]
    pub fn ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view: &mut SpaceView,
    ) -> bool {
        // HACK: We want to dim everything a little bit. So draw one large rectangle over everything.
        let painter = egui::Painter::new(
            ui.ctx().clone(),
            egui::LayerId::new(egui::Order::PanelResizeLine, egui::Id::new("DimLayer")),
            egui::Rect::EVERYTHING,
        );
        painter.add(egui::Shape::rect_filled(
            ui.ctx().screen_rect(),
            egui::Rounding::none(),
            egui::Color32::from_black_alpha(128),
        ));

        // Close window using escape button.
        let mut open = ui.input(|i| !i.key_pressed(egui::Key::Escape));
        let title = "Add/remove Entities";

        let response = egui::Window::new(title)
            // TODO(andreas): Doesn't center properly. `pivot(Align2::CENTER_CENTER)` seems to be broken. Also, should reset every time
            .default_pos(ui.ctx().screen_rect().center())
            .collapsible(false)
            .frame(ctx.re_ui.panel_frame())
            .title_bar(false) // We do a custom title bar for better adhoc styling.
            .show(ui.ctx(), |ui| {
                title_bar(ctx.re_ui, ui, title, &mut open);
                add_entities_ui(ctx, ui, space_view);
            });

        // HACK: Fake modality - any click outside causes the window to close.
        let cursor_was_over_window = if let Some(response) = response {
            if let Some(interact_pos) = ui.input(|i| i.pointer.interact_pos()) {
                response.response.rect.contains(interact_pos)
            } else {
                false
            }
        } else {
            false
        };
        if !cursor_was_over_window && ui.input(|i| i.pointer.any_pressed()) {
            open = false;
        }

        open
    }
}

fn add_entities_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, space_view: &mut SpaceView) {
    let spaces_info = SpaceInfoCollection::new(&ctx.log_db.entity_db);
    // TODO(andreas): remove use space_view.root_path, just show everything
    if let Some(tree) = ctx.log_db.entity_db.tree.subtree(&space_view.root_path) {
        let entities_add_info = create_entity_add_info(ctx, tree, space_view, &spaces_info);

        add_entities_tree_ui(
            ctx,
            ui,
            &spaces_info,
            &tree.path.to_string(),
            tree,
            space_view,
            &entities_add_info,
        );
    }
}

fn add_entities_tree_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
    name: &str,
    tree: &EntityTree,
    space_view: &mut SpaceView,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    if tree.is_leaf() {
        add_entities_line_ui(
            ctx,
            ui,
            spaces_info,
            &format!("🔹 {name}"),
            &tree.path,
            space_view,
            entities_add_info,
        );
    } else {
        let level = tree.path.len();
        let default_open = space_view.space_path.is_descendant_of(&tree.path)
            || tree.children.len() <= 3
            || level < 1;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.id().with(name),
            default_open,
        )
        .show_header(ui, |ui| {
            add_entities_line_ui(
                ctx,
                ui,
                spaces_info,
                name,
                &tree.path,
                space_view,
                entities_add_info,
            );
        })
        .body(|ui| {
            for (path_comp, child_tree) in tree.children.iter().sorted_by_key(|(_, child_tree)| {
                // Put descendants of the space path always first
                i32::from(
                    !(child_tree.path == space_view.space_path
                        || child_tree.path.is_descendant_of(&space_view.space_path)),
                )
            }) {
                add_entities_tree_ui(
                    ctx,
                    ui,
                    spaces_info,
                    &path_comp.to_string(),
                    child_tree,
                    space_view,
                    entities_add_info,
                );
            }
        });
    };
}

fn add_entities_line_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
    name: &str,
    entity_path: &EntityPath,
    space_view: &mut SpaceView,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    ui.horizontal(|ui| {
        let space_view_id = if space_view.data_blueprint.contains_entity(entity_path) {
            Some(space_view.id)
        } else {
            None
        };
        let add_info = entities_add_info.get(entity_path).unwrap();

        // To be safe, let's put the add/remove button outside of this enabled block for the name display.
        // (i.e. if for some reason it was already added, we want it to be still be removable!)
        ui.add_enabled_ui(add_info.can_add_self_or_any_child, |ui| {
            let widget_text = if entity_path == &space_view.space_path {
                egui::RichText::new(name).strong()
            } else {
                egui::RichText::new(name)
            };
            let response = ctx.instance_path_button_to(
                ui,
                space_view_id,
                &InstancePath::entity_splat(entity_path.clone()),
                widget_text,
            );
            if entity_path == &space_view.space_path {
                response.highlight();
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let entity_tree = &ctx.log_db.entity_db.tree;

            if space_view.data_blueprint.contains_entity(entity_path) {
                if ctx
                    .re_ui
                    .small_icon_button(ui, &re_ui::icons::REMOVE)
                    .on_hover_text("Remove this Entity and all its children from the Space View")
                    .clicked()
                {
                    // Remove all entities at and under this path
                    entity_tree
                        .subtree(entity_path)
                        .unwrap()
                        .visit_children_recursively(&mut |path: &EntityPath| {
                            space_view.data_blueprint.remove_entity(path);
                            space_view.entities_determined_by_user = true;
                        });
                }
            } else {
                let response = ui
                    .add_enabled_ui(add_info.can_add_self_or_any_child, |ui| {
                        let response = ctx
                            .re_ui
                            .small_icon_button(ui, &re_ui::icons::ADD)
                            .on_hover_text(
                                "Add this Entity and all its children to the Space View",
                            );
                        if response.clicked() {
                            // Insert the entity it space_view and all its children as far as they haven't been added yet
                            let mut entities = Vec::new();
                            entity_tree
                                .subtree(entity_path)
                                .unwrap()
                                .visit_children_recursively(&mut |path: &EntityPath| {
                                    if has_visualization_for_category(
                                        ctx,
                                        space_view.category,
                                        path,
                                    ) && !space_view.data_blueprint.contains_entity(path)
                                        && spaces_info
                                            .is_reachable_by_transform(path, &space_view.space_path)
                                            .is_ok()
                                    {
                                        entities.push(path.clone());
                                    }
                                });
                            space_view
                                .data_blueprint
                                .insert_entities_according_to_hierarchy(
                                    entities.iter(),
                                    &space_view.space_path,
                                );
                            space_view.entities_determined_by_user = true;
                        }
                    })
                    .response;

                if let Some(cannot_add_reason) = &add_info.cannot_add_reason {
                    response.on_hover_text(cannot_add_reason);
                }
            }
        });
    });
}

#[derive(Default)]
struct EntityAddInfo {
    _categories: enumset::EnumSet<ViewCategory>, // TODO(andreas): Should use this to display icons
    cannot_add_reason: Option<String>,
    can_add_self_or_any_child: bool,
}

fn create_entity_add_info(
    ctx: &mut ViewerContext<'_>,
    tree: &EntityTree,
    space_view: &mut SpaceView,
    spaces_info: &SpaceInfoCollection,
) -> IntMap<EntityPath, EntityAddInfo> {
    let mut meta_data: IntMap<EntityPath, EntityAddInfo> = IntMap::default();

    tree.visit_children_recursively(&mut |entity_path| {
        let categories = categorize_entity_path(Timeline::log_time(), ctx.log_db, entity_path);
        let cannot_add_reason = if categories.contains(space_view.category) {
            spaces_info
                .is_reachable_by_transform(entity_path, &space_view.space_path)
                .map_err(|reason| reason.to_string())
                .err()
        } else if categories.is_empty() {
            Some("Entity does not have any components".to_owned())
        } else {
            Some(format!(
                "Entity can't be displayed by this type of Space View {}",
                space_view.category
            ))
        };

        if cannot_add_reason.is_none() {
            // Mark parents that there is something that can be added.
            let mut path = entity_path.clone();
            while let Some(parent) = path.parent() {
                meta_data
                    .entry(parent.clone())
                    .or_default()
                    .can_add_self_or_any_child = true;
                path = parent;
            }
        }

        let can_add_self_or_any_child = cannot_add_reason.is_none();
        meta_data.insert(
            entity_path.clone(),
            EntityAddInfo {
                _categories: categories,
                cannot_add_reason,
                can_add_self_or_any_child,
            },
        );
    });

    meta_data
}

fn title_bar(re_ui: &re_ui::ReUi, ui: &mut egui::Ui, title: &str, open: &mut bool) {
    ui.horizontal(|ui| {
        ui.heading(title);

        ui.add_space(16.0);

        let mut ui = ui.child_ui(
            ui.max_rect(),
            egui::Layout::right_to_left(egui::Align::Center),
        );
        if re_ui
            .small_icon_button(&mut ui, &re_ui::icons::CLOSE)
            .clicked()
        {
            *open = false;
        }
    });
    ui.separator();
}

fn has_visualization_for_category(
    ctx: &ViewerContext<'_>,
    category: ViewCategory,
    entity_path: &EntityPath,
) -> bool {
    let log_db = &ctx.log_db;
    categorize_entity_path(Timeline::log_time(), log_db, entity_path).contains(category)
}
