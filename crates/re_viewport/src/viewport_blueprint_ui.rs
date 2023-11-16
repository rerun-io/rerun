use egui::{Response, Ui};
use itertools::Itertools;

use re_data_store::InstancePath;
use re_data_ui::item_ui;
use re_space_view::{DataBlueprintGroup, DataQuery as _};
use re_ui::list_item::ListItem;
use re_ui::ReUi;
use re_viewer_context::{
    DataBlueprintGroupHandle, HoverHighlight, Item, SpaceViewId, ViewerContext,
};

use crate::{
    space_view_heuristics::{all_possible_space_views, identify_entities_per_system_per_class},
    viewport_blueprint::TreeActions,
    SpaceInfoCollection, SpaceViewBlueprint, ViewportBlueprint,
};

impl ViewportBlueprint<'_> {
    /// Show the blueprint panel tree view.
    pub fn tree_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .id_source("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                if let Some(root) = self.tree.root() {
                    self.tile_ui(ctx, ui, root);
                }
            });

        let TreeActions { focus_tab, remove } = std::mem::take(&mut self.deferred_tree_actions);

        if let Some(focus_tab) = &focus_tab {
            let found = self.tree.make_active(|tile| match tile {
                egui_tiles::Tile::Pane(space_view_id) => space_view_id == focus_tab,
                egui_tiles::Tile::Container(_) => false,
            });
            re_log::trace!("Found tab {focus_tab}: {found}");
        }

        for tile_id in remove {
            for tile in self.tree.tiles.remove_recursively(tile_id) {
                if let egui_tiles::Tile::Pane(space_view_id) = tile {
                    self.remove(&space_view_id);
                }
            }

            if Some(tile_id) == self.tree.root {
                self.tree.root = None;
            }
        }
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
    ) {
        // Temporarily remove the tile so we don't get borrow-checker fights:
        let Some(mut tile) = self.tree.tiles.remove(tile_id) else {
            return;
        };

        match &mut tile {
            egui_tiles::Tile::Container(container) => {
                self.container_tree_ui(ctx, ui, tile_id, container);
            }
            egui_tiles::Tile::Pane(space_view_id) => {
                // A space view
                self.space_view_entry_ui(ctx, ui, tile_id, space_view_id);
            }
        };

        self.tree.tiles.insert(tile_id, tile);
    }

    fn container_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        container: &egui_tiles::Container,
    ) {
        if let Some(child_id) = container.only_child() {
            // Maybe a tab container with only one child - collapse it in the tree view to make it more easily understood.
            // This means we won't be showing the visibility button of the parent container,
            // so if the child is made invisible, we should do the same for the parent.
            let child_is_visible = self.tree.is_visible(child_id);
            self.tree.set_visible(tile_id, child_is_visible);
            return self.tile_ui(ctx, ui, child_id);
        }

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let mut remove = false;

        let default_open = true;

        ListItem::new(ctx.re_ui, format!("{:?}", container.kind()))
            .subdued(true)
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, true, &mut visible);
                visibility_changed = vis_response.changed();

                let remove_response = remove_button_ui(re_ui, ui, "Remove container");
                remove = remove_response.clicked();

                remove_response | vis_response
            })
            .show_collapsing(ui, ui.id().with(tile_id), default_open, |_, ui| {
                for &child in container.children() {
                    self.tile_ui(ctx, ui, child);
                }
            });

        if remove {
            self.mark_user_interaction();
            self.deferred_tree_actions.remove.push(tile_id);
        }

        if visibility_changed {
            if self.auto_layout {
                re_log::trace!("Container visibility changed - will no longer auto-layout");
            }

            self.auto_layout = false; // Keep `auto_space_views` enabled.
            self.tree.set_visible(tile_id, visible);
        }
    }

    fn space_view_entry_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        space_view_id: &SpaceViewId,
    ) {
        let Some(space_view) = self.space_views.get_mut(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            self.deferred_tree_actions.remove.push(tile_id);
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let visible_child = visible;
        let item = Item::SpaceView(space_view.id);

        let root_group = space_view.contents.root_group();
        let default_open = Self::default_open_for_group(root_group);
        let collapsing_header_id = ui.id().with(space_view.id);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let response = ListItem::new(ctx.re_ui, space_view.display_name.clone())
            .selected(ctx.selection().contains(&item))
            .subdued(!visible)
            .force_hovered(is_item_hovered)
            .with_icon(space_view.class(ctx.space_view_class_registry).icon())
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, true, &mut visible);
                visibility_changed = vis_response.changed();

                let response = remove_button_ui(re_ui, ui, "Remove Space View from the Viewport");
                if response.clicked() {
                    self.deferred_tree_actions.remove.push(tile_id);
                }

                response | vis_response
            })
            .show_collapsing(ui, collapsing_header_id, default_open, |_, ui| {
                Self::space_view_blueprint_ui(
                    ctx,
                    ui,
                    space_view.contents.root_handle(),
                    space_view,
                    visible_child,
                );
            })
            .item_response
            .on_hover_text("Space View");

        if response.clicked() {
            self.deferred_tree_actions.focus_tab = Some(space_view.id);
        }

        item_ui::select_hovered_on_click(ctx, &response, &[item]);

        if visibility_changed {
            if self.auto_layout {
                re_log::trace!("Space view visibility changed - will no longer auto-layout");
            }

            self.auto_layout = false; // Keep `auto_space_views` enabled.
            self.tree.set_visible(tile_id, visible);
        }
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

        // TODO(jleibs): We should use `query` instead of `resolve` in this function, but doing so
        // requires changing the view layout a little bit, so holding off on that for now.
        let top_data_result = space_view
            .contents
            .resolve(space_view, ctx, &group.group_path);

        let group_is_visible = top_data_result.resolved_properties.visible && space_view_visible;

        for entity_path in &entities {
            if entity_path.is_root() {
                continue;
            }

            let is_selected = ctx.selection().contains(&Item::InstancePath(
                Some(space_view.id),
                InstancePath::entity_splat(entity_path.clone()),
            ));

            let item = Item::InstancePath(
                Some(space_view.id),
                InstancePath::entity_splat(entity_path.clone()),
            );
            let is_item_hovered =
                ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

            let data_result = space_view.contents.resolve(space_view, ctx, entity_path);

            let mut properties = data_result
                .individual_properties
                .clone()
                .unwrap_or_default();

            let name = entity_path.iter().last().unwrap().to_string();
            let label = format!("🔹 {name}");
            let response = ListItem::new(ctx.re_ui, label)
                .selected(is_selected)
                .subdued(!group_is_visible || !properties.visible)
                .force_hovered(is_item_hovered)
                .with_buttons(|re_ui, ui| {
                    let vis_response =
                        visibility_button_ui(re_ui, ui, group_is_visible, &mut properties.visible);
                    data_result.save_override(Some(properties), ctx);

                    let response = remove_button_ui(re_ui, ui, "Remove Entity from the Space View");
                    if response.clicked() {
                        space_view.contents.remove_entity(entity_path);
                        space_view.entities_determined_by_user = true;
                    }

                    response | vis_response
                })
                .show(ui)
                .on_hover_ui(|ui| {
                    re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, entity_path);
                });

            item_ui::select_hovered_on_click(ctx, &response, &[item]);
        }

        for child_group_handle in &children {
            let Some(child_group) = space_view.contents.group(*child_group_handle) else {
                debug_assert!(
                    false,
                    "Data blueprint group {group_name} has an invalid child"
                );
                continue;
            };

            let item = Item::DataBlueprintGroup(space_view.id, *child_group_handle);
            let is_selected = ctx.selection().contains(&item);
            let is_item_hovered =
                ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

            let mut remove_group = false;
            let default_open = Self::default_open_for_group(child_group);

            let child_data_result =
                space_view
                    .contents
                    .resolve(space_view, ctx, &child_group.group_path);

            let mut child_properties = child_data_result
                .individual_properties
                .clone()
                .unwrap_or_default();

            let response = ListItem::new(ctx.re_ui, child_group.display_name.clone())
                .selected(is_selected)
                .subdued(!child_properties.visible || !group_is_visible)
                .force_hovered(is_item_hovered)
                .with_icon(&re_ui::icons::CONTAINER)
                .with_buttons(|re_ui, ui| {
                    let vis_response = visibility_button_ui(
                        re_ui,
                        ui,
                        group_is_visible,
                        &mut child_properties.visible,
                    );

                    let response = remove_button_ui(
                        re_ui,
                        ui,
                        "Remove Group and all its children from the Space View",
                    );
                    if response.clicked() {
                        remove_group = true;
                    }

                    response | vis_response
                })
                .show_collapsing(
                    ui,
                    ui.id().with(child_group_handle),
                    default_open,
                    |_, ui| {
                        Self::space_view_blueprint_ui(
                            ctx,
                            ui,
                            *child_group_handle,
                            space_view,
                            space_view_visible,
                        );
                    },
                )
                .item_response
                .on_hover_text("Group");

            child_data_result.save_override(Some(child_properties), ctx);

            item_ui::select_hovered_on_click(ctx, &response, &[item]);

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

        ui.menu_image_button(
            re_ui::icons::ADD
                .as_image()
                .fit_to_exact_size(re_ui::ReUi::small_icon_size()),
            |ui| {
                ui.style_mut().wrap = Some(false);

                let entities_per_system_per_class = identify_entities_per_system_per_class(ctx);
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
            },
        )
        .response
        .on_hover_text("Add new Space View");
    }
}

// ----------------------------------------------------------------------------

fn remove_button_ui(re_ui: &ReUi, ui: &mut Ui, tooltip: &str) -> Response {
    re_ui
        .small_icon_button(ui, &re_ui::icons::REMOVE)
        .on_hover_text(tooltip)
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
