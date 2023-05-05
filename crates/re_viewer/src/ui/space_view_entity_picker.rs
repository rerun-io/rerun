use itertools::Itertools;
use nohash_hasher::IntMap;
use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityTree, InstancePath};
use re_data_ui::item_ui;
use re_viewer_context::{SpaceViewId, ViewerContext};

use crate::misc::space_info::SpaceInfoCollection;

use super::{
    view_category::{categorize_entity_path, ViewCategory},
    SpaceView,
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
        // This function fakes a modal window, since egui doesn't have them yet: https://github.com/emilk/egui/issues/686

        // In particular, we dim the background and close the window when the user clicks outside it
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
            .default_height(640.0)
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                inner_margin: re_ui::ReUi::view_padding().into(),
                ..Default::default()
            })
            // We do a custom title bar for better adhoc styling.
            // TODO(andreas): Ideally the default title bar would already adhere to that style
            .title_bar(false)
            .show(ui.ctx(), |ui| {
                title_bar(ctx.re_ui, ui, title, &mut open);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    add_entities_ui(ctx, ui, space_view);
                });
            });

        // Any click outside causes the window to close.
        let cursor_was_over_window = response
            .and_then(|response| {
                ui.input(|i| i.pointer.interact_pos())
                    .map(|interact_pos| response.response.rect.contains(interact_pos))
            })
            .unwrap_or(false);
        if !cursor_was_over_window && ui.input(|i| i.pointer.any_pressed()) {
            open = false;
        }

        open
    }
}

fn add_entities_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, space_view: &mut SpaceView) {
    let spaces_info = SpaceInfoCollection::new(&ctx.log_db.entity_db);
    let tree = &ctx.log_db.entity_db.tree;
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
            tree,
            space_view,
            entities_add_info,
        );
    } else {
        let level = tree.path.len();
        let default_open = space_view.space_path.is_descendant_of(&tree.path)
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
                spaces_info,
                name,
                tree,
                space_view,
                entities_add_info,
            );
        })
        .body(|ui| {
            for (path_comp, child_tree) in tree.children.iter().sorted_by_key(|(_, child_tree)| {
                // Put descendants of the space path always first
                let put_first = child_tree.path == space_view.space_path
                    || child_tree.path.is_descendant_of(&space_view.space_path);
                !put_first
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
    entity_tree: &EntityTree,
    space_view: &mut SpaceView,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    ui.horizontal(|ui| {
        let entity_path = &entity_tree.path;

        let space_view_id = if space_view.data_blueprint.contains_entity(entity_path) {
            Some(space_view.id)
        } else {
            None
        };
        let add_info = entities_add_info.get(entity_path).unwrap();

        ui.add_enabled_ui(add_info.can_add_self_or_descendant.is_compatible(), |ui| {
            let widget_text = if entity_path == &space_view.space_path {
                egui::RichText::new(name).strong()
            } else {
                egui::RichText::new(name)
            };
            let response = item_ui::instance_path_button_to(
                ctx,
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
            if space_view.data_blueprint.contains_entity(entity_path) {
                if ctx
                    .re_ui
                    .small_icon_button(ui, &re_ui::icons::REMOVE)
                    .on_hover_text("Remove this Entity and all its descendants from the Space View")
                    .clicked()
                {
                    space_view.remove_entity_subtree(entity_tree);
                }
            } else {
                ui.add_enabled_ui(
                    add_info
                        .can_add_self_or_descendant
                        .is_compatible_and_missing(),
                    |ui| {
                        let response = ctx.re_ui.small_icon_button(ui, &re_ui::icons::ADD);
                        if response.clicked() {
                            space_view.add_entity_subtree(entity_tree, spaces_info, ctx.log_db);
                        }

                        if add_info
                            .can_add_self_or_descendant
                            .is_compatible_and_missing()
                        {
                            if add_info.can_add.is_compatible_and_missing() {
                                response.on_hover_text(
                                    "Add this Entity and all its descendants to the Space View",
                                );
                            } else {
                                response.on_hover_text(
                                    "Add descendants of this Entity to the Space View",
                                );
                            }
                        } else if let CanAddToSpaceView::No { reason } = &add_info.can_add {
                            response.on_disabled_hover_text(reason);
                        }
                    },
                );
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
struct EntityAddInfo {
    #[allow(dead_code)]
    categories: enumset::EnumSet<ViewCategory>,
    can_add: CanAddToSpaceView,
    can_add_self_or_descendant: CanAddToSpaceView,
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
        let can_add: CanAddToSpaceView = if categories.contains(space_view.category) {
            match spaces_info.is_reachable_by_transform(entity_path, &space_view.space_path) {
                Ok(()) => CanAddToSpaceView::Compatible {
                    already_added: space_view.data_blueprint.contains_entity(entity_path),
                },
                Err(reason) => CanAddToSpaceView::No {
                    reason: reason.to_string(),
                },
            }
        } else if categories.is_empty() {
            CanAddToSpaceView::No {
                reason: "Entity does not have any components".to_owned(),
            }
        } else {
            CanAddToSpaceView::No {
                reason: format!(
                    "Entity can't be displayed by this type of Space View ({})",
                    space_view.category
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
                categories,
                can_add,
                can_add_self_or_descendant,
            },
        );
    });

    meta_data
}

fn title_bar(re_ui: &re_ui::ReUi, ui: &mut egui::Ui, title: &str, open: &mut bool) {
    ui.horizontal(|ui| {
        ui.strong(title);

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
