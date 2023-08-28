use itertools::Itertools;
use re_data_store::InstancePath;
use re_data_ui::item_ui;
use re_space_view::DataBlueprintGroup;
use re_viewer_context::{DataBlueprintGroupHandle, Item, SpaceViewId, ViewerContext};

use crate::{
    space_view_heuristics::{all_possible_space_views, default_entities_per_system_per_class},
    SpaceInfoCollection, SpaceViewBlueprint, ViewportBlueprint,
};

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
enum TreeAction {
    Keep,
    Remove,
}

impl ViewportBlueprint<'_> {
    /// Show the blueprint panel tree view.
    pub fn tree_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .id_source("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                if let Some(root) = self.tree.root() {
                    match self.tile_ui(ctx, ui, root) == TreeAction::Remove {
                        true => {
                            self.tree.root = None;
                        }
                        false => (),
                    }
                }
            });
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_group(group: &DataBlueprintGroup) -> bool {
        let num_children = group.children.len() + group.entities.len();
        2 <= num_children && num_children <= 3
    }

    fn tile_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
    ) -> TreeAction {
        // Temporarily remove the tile so we don't get borrow checker fights:
        let Some(mut tile) = self.tree.tiles.remove(tile_id) else {
            return TreeAction::Remove;
        };

        let action = match &mut tile {
            egui_tiles::Tile::Container(container) => {
                self.container_tree_ui(ctx, ui, tile_id, container)
            }
            egui_tiles::Tile::Pane(space_view_id) => {
                // A space view
                self.space_view_entry_ui(ctx, ui, tile_id, space_view_id)
            }
        };

        self.tree.tiles.insert(tile_id, tile);

        if action == TreeAction::Remove {
            for tile in self.tree.tiles.remove_recursively(tile_id) {
                if let egui_tiles::Tile::Pane(space_view_id) = tile {
                    self.remove(&space_view_id);
                }
            }
        }

        action
    }

    fn container_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        container: &mut egui_tiles::Container,
    ) -> TreeAction {
        if let Some(child_id) = container.only_child() {
            // Maybe a tab container with only one child - collapse it in the tree view to make it more easily understood.
            // This means we won't be showing the visibility button of the parent container,
            // so if the child is made invisible, we should do the same for the parent.
            let child_is_visible = self.tree.is_visible(child_id);
            self.tree.set_visible(tile_id, child_is_visible);
            return self.tile_ui(ctx, ui, child_id);
        }

        let mut visibility_changed = false;
        let mut action = TreeAction::Keep;
        let mut visible = self.tree.is_visible(tile_id);

        let default_open = true;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            egui::Id::new((tile_id, "tree")),
            default_open,
        )
        .show_header(ui, |ui| {
            blueprint_row_with_buttons(
                ctx.re_ui,
                ui,
                true,
                visible,
                false,
                |ui| ui.label(format!("{:?}", container.kind())),
                |re_ui, ui| {
                    visibility_changed =
                        visibility_button_ui(re_ui, ui, true, &mut visible).changed();
                    if re_ui
                        .small_icon_button(ui, &re_ui::icons::REMOVE)
                        .on_hover_text("Remove container")
                        .clicked()
                    {
                        action = TreeAction::Remove;
                    }
                },
            );
        })
        .body(|ui| container.retain(|child| self.tile_ui(ctx, ui, child) == TreeAction::Keep));

        if visibility_changed {
            self.has_been_user_edited = true;
            self.tree.set_visible(tile_id, visible);
        }

        action
    }

    fn space_view_entry_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        space_view_id: &SpaceViewId,
    ) -> TreeAction {
        let Some(space_view) = self.space_views.get_mut(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            return TreeAction::Remove;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        let mut visibility_changed = false;
        let mut action = TreeAction::Keep;
        let mut visible = self.tree.is_visible(tile_id);
        let item = Item::SpaceView(space_view.id);
        let is_selected = ctx.selection().contains(&item);

        let root_group = space_view.contents.root_group();
        let default_open = Self::default_open_for_group(root_group);
        let collapsing_header_id = ui.id().with(space_view.id);
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            blueprint_row_with_buttons(
                ctx.re_ui,
                ui,
                true,
                visible,
                is_selected,
                |ui| {
                    let response = crate::item_ui::space_view_button(ctx, ui, space_view);
                    if response.clicked() {
                        focus_tab(&mut self.tree, space_view_id);
                    }
                    response
                },
                |re_ui, ui| {
                    visibility_changed =
                        visibility_button_ui(re_ui, ui, true, &mut visible).changed();
                    if re_ui
                        .small_icon_button(ui, &re_ui::icons::REMOVE)
                        .on_hover_text("Remove Space View from the Viewport")
                        .clicked()
                    {
                        action = TreeAction::Remove;
                    }
                },
            );
        })
        .body(|ui| {
            Self::space_view_blueprint_ui(
                ctx,
                ui,
                space_view.contents.root_handle(),
                space_view,
                visible,
            );
        });

        if visibility_changed {
            self.has_been_user_edited = true;
            self.tree.set_visible(tile_id, visible);
        }

        if action == TreeAction::Remove {
            self.remove(space_view_id);
        }

        action
    }

    fn space_view_blueprint_ui(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        group_handle: DataBlueprintGroupHandle,
        space_view: &mut SpaceViewBlueprint,
        space_view_visible: bool,
    ) {
        let Some(group) = space_view.contents.group(group_handle) else {
            debug_assert!(false, "Invalid group handle in blueprint group tree");
            return;
        };

        // TODO(andreas): These clones are workarounds against borrowing multiple times from space_view_blueprint_ui.
        let children = group.children.clone();
        let entities = group.entities.clone();
        let group_name = group.display_name.clone();
        let group_is_visible = group.properties_projected.visible && space_view_visible;

        for entity_path in &entities {
            if entity_path.is_root() {
                continue;
            }

            let is_selected = ctx.selection().contains(&Item::InstancePath(
                Some(space_view.id),
                InstancePath::entity_splat(entity_path.clone()),
            ));

            ui.horizontal(|ui| {
                let mut properties = space_view
                    .contents
                    .data_blueprints_individual()
                    .get(entity_path);
                blueprint_row_with_buttons(
                    ctx.re_ui,
                    ui,
                    group_is_visible,
                    properties.visible,
                    is_selected,
                    |ui| {
                        let name = entity_path.iter().last().unwrap().to_string();
                        let label = format!("🔹 {name}");
                        re_data_ui::item_ui::data_blueprint_button_to(
                            ctx,
                            ui,
                            label,
                            space_view.id,
                            entity_path,
                        )
                    },
                    |re_ui, ui| {
                        if visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut properties.visible,
                        )
                        .changed()
                        {
                            space_view
                                .contents
                                .data_blueprints_individual()
                                .set(entity_path.clone(), properties);
                        }
                        if re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove Entity from the Space View")
                            .clicked()
                        {
                            space_view.contents.remove_entity(entity_path);
                            space_view.entities_determined_by_user = true;
                        }
                    },
                );
            });
        }

        for child_group_handle in &children {
            let Some(child_group) = space_view.contents.group_mut(*child_group_handle) else {
                debug_assert!(
                    false,
                    "Data blueprint group {group_name} has an invalid child"
                );
                continue;
            };

            let is_selected = ctx.selection().contains(&Item::DataBlueprintGroup(
                space_view.id,
                *child_group_handle,
            ));

            let mut remove_group = false;
            let default_open = Self::default_open_for_group(child_group);
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(child_group_handle),
                default_open,
            )
            .show_header(ui, |ui| {
                blueprint_row_with_buttons(
                    ctx.re_ui,
                    ui,
                    group_is_visible,
                    child_group.properties_individual.visible,
                    is_selected,
                    |ui| {
                        item_ui::data_blueprint_group_button_to(
                            ctx,
                            ui,
                            child_group.display_name.clone(),
                            space_view.id,
                            *child_group_handle,
                        )
                    },
                    |re_ui, ui| {
                        visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut child_group.properties_individual.visible,
                        );
                        if re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove Group and all its children from the Space View")
                            .clicked()
                        {
                            remove_group = true;
                        }
                    },
                );
            })
            .body(|ui| {
                Self::space_view_blueprint_ui(
                    ctx,
                    ui,
                    *child_group_handle,
                    space_view,
                    space_view_visible,
                );
            });
            if remove_group {
                space_view.contents.remove_group(*child_group_handle);
                space_view.entities_determined_by_user = true;
            }
        }
    }

    pub fn add_new_spaceview_button_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        #![allow(clippy::collapsible_if)]

        let icon_image = ctx.re_ui.icon_image(&re_ui::icons::ADD);
        let texture_id = icon_image.texture_id(ui.ctx());
        ui.menu_image_button(texture_id, re_ui::ReUi::small_icon_size(), |ui| {
            ui.style_mut().wrap = Some(false);

            let entities_per_system_per_class = default_entities_per_system_per_class(ctx);
            for space_view in
                all_possible_space_views(ctx, spaces_info, &entities_per_system_per_class)
                    .into_iter()
                    .sorted_by_key(|space_view| space_view.space_origin.to_string())
            {
                if ctx
                    .re_ui
                    .selectable_label_with_icon(
                        ui,
                        space_view.class(ctx.space_view_class_registry).icon(),
                        if space_view.space_origin.is_root() {
                            space_view.display_name.clone()
                        } else {
                            space_view.space_origin.to_string()
                        },
                        false,
                    )
                    .clicked()
                {
                    ui.close_menu();
                    let new_space_view_id = self.add_space_view(space_view);
                    ctx.set_single_selection(&Item::SpaceView(new_space_view_id));
                }
            }
        })
        .response
        .on_hover_text("Add new Space View");
    }
}

// ----------------------------------------------------------------------------

fn focus_tab(tree: &mut egui_tiles::Tree<SpaceViewId>, tab: &SpaceViewId) {
    tree.make_active(|tile| match tile {
        egui_tiles::Tile::Pane(space_view_id) => space_view_id == tab,
        egui_tiles::Tile::Container(_) => false,
    });
}

/// Show a single button (`add_content`), justified,
/// and show a visibility button if the row is hovered.
///
/// Returns true if visibility changed.
#[allow(clippy::fn_params_excessive_bools)]
fn blueprint_row_with_buttons(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    enabled: bool,
    visible: bool,
    selected: bool,
    add_content: impl FnOnce(&mut egui::Ui) -> egui::Response,
    add_on_hover_buttons: impl FnOnce(&re_ui::ReUi, &mut egui::Ui),
) {
    let where_to_add_hover_rect = ui.painter().add(egui::Shape::Noop);

    // Make the main button span the whole width to make it easier to click:
    let main_button_response = ui
        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.style_mut().wrap = Some(false);

            // Turn off the background color of hovered buttons.
            // Why? Because we add a manual hover-effect later.
            // Why? Because we want that hover-effect even when only the visibility button is hovered.
            let visuals = ui.visuals_mut();
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.open.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.open.bg_fill = egui::Color32::TRANSPARENT;

            if ui
                .interact(ui.max_rect(), ui.id(), egui::Sense::hover())
                .hovered()
            {
                // Clip the main button so that the on-hover buttons have room to cover it.
                // Ideally we would only clip the button _text_, not the button background, but that's not possible.
                let mut clip_rect = ui.max_rect();
                let on_hover_buttons_width = 36.0;
                clip_rect.max.x -= on_hover_buttons_width;
                ui.set_clip_rect(clip_rect);
            }

            if !visible || !enabled {
                // Dim the appearance of things added by `add_content`:
                let widget_visuals = &mut ui.visuals_mut().widgets;

                fn dim_color(color: &mut egui::Color32) {
                    *color = color.gamma_multiply(0.5);
                }
                dim_color(&mut widget_visuals.noninteractive.fg_stroke.color);
                dim_color(&mut widget_visuals.inactive.fg_stroke.color);
            }

            add_content(ui)
        })
        .inner;

    let main_button_rect = main_button_response.rect;

    // We check the same rectangle as the main button,
    // but we will also catch hovers on the visibility button (if any).
    let button_hovered = ui
        .interact(main_button_rect, ui.id(), egui::Sense::hover())
        .hovered();
    if button_hovered {
        // Just put the buttons on top of the existing ui:
        let mut ui = ui.child_ui(
            ui.max_rect(),
            egui::Layout::right_to_left(egui::Align::Center),
        );
        add_on_hover_buttons(re_ui, &mut ui);
    }

    // The main button might have been highlighted because what it was referring
    // to was hovered somewhere else, and then we also want it highlighted here.
    if button_hovered || main_button_response.highlighted() || selected {
        // Highlight the row:
        let visuals = ui.visuals().widgets.hovered;

        let bg_fill = if selected {
            ui.style().visuals.selection.bg_fill
        } else {
            visuals.bg_fill
        };
        let hover_rect = main_button_rect.expand(visuals.expansion);
        ui.painter().set(
            where_to_add_hover_rect,
            egui::Shape::rect_filled(hover_rect, visuals.rounding, bg_fill),
        );
    }
}

fn visibility_button_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    enabled: bool,
    visible: &mut bool,
) -> egui::Response {
    ui.set_enabled(enabled);
    re_ui
        .visibility_toggle_button(ui, visible)
        .on_hover_text("Toggle visibility")
        .on_disabled_hover_text("A parent is invisible")
}
