use egui::{Response, Ui};
use itertools::Itertools;

use re_data_store::InstancePath;
use re_data_ui::item_ui;
use re_log_types::{EntityPath, EntityPathExpr};
use re_space_view::DataQueryBlueprint;
use re_ui::list_item::ListItem;
use re_ui::ReUi;
use re_viewer_context::{
    DataQueryResult, DataResultHandle, DataResultNode, HoverHighlight, Item, SpaceViewId,
    ViewerContext,
};

use crate::{
    space_view_heuristics::all_possible_space_views, SpaceInfoCollection, SpaceViewBlueprint,
    Viewport,
};

impl Viewport<'_, '_> {
    /// Show the blueprint panel tree view.
    pub fn tree_ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .id_source("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                ctx.re_ui.panel_content(ui, |_, ui| {
                    if let Some(root) = self.tree.root() {
                        self.tile_ui(ctx, ui, root);
                    }
                });
            });

        // At the end of the Tree-UI, we can safely apply deferred actions.

        let mut reset = std::mem::take(&mut self.deferred_tree_actions.reset);
        let create = std::mem::take(&mut self.deferred_tree_actions.create);
        let mut focus_tab = std::mem::take(&mut self.deferred_tree_actions.focus_tab);
        let remove = std::mem::take(&mut self.deferred_tree_actions.remove);

        for space_view in &create {
            if self.blueprint.auto_layout {
                // Re-run the auto-layout next frame:
                re_log::trace!(
                    "Added a space view with no user edits yet - will re-run auto-layout"
                );

                reset = true;
            } else if let Some(root_id) = self.tree.root {
                let tile_id = self.tree.tiles.insert_pane(*space_view);
                if let Some(egui_tiles::Tile::Container(container)) =
                    self.tree.tiles.get_mut(root_id)
                {
                    re_log::trace!("Inserting new space view into root container");
                    container.add_child(tile_id);
                } else {
                    re_log::trace!("Root was not a container - will re-run auto-layout");
                    reset = true;
                }
            } else {
                re_log::trace!("No root found - will re-run auto-layout");
            }

            focus_tab = Some(*space_view);
        }

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
                    self.tree.tiles.remove(tile_id);
                    self.blueprint.remove_space_view(&space_view_id, ctx);
                }
            }

            if Some(tile_id) == self.tree.root {
                self.tree.root = None;
            }
        }

        if self.tree.is_empty() && !self.blueprint.space_views.is_empty() {
            re_log::trace!("Tree is empty - will re-run auto-layout");
            reset = true;
        }

        if reset {
            re_log::trace!("Resetting viewport tree");
            self.tree = super::auto_layout::tree_from_space_views(
                ctx.space_view_class_registry,
                &self.blueprint.space_views,
            );
        }
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_data_result(group: &DataResultNode) -> bool {
        let num_children = group.children.len();
        2 <= num_children && num_children <= 3
    }

    fn tile_ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, tile_id: egui_tiles::TileId) {
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
        ctx: &ViewerContext<'_>,
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

        let item = Item::Container(tile_id);

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let mut remove = false;

        let default_open = true;

        let response = ListItem::new(ctx.re_ui, format!("{:?}", container.kind()))
            .subdued(true)
            .selected(ctx.selection().contains(&item))
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
            })
            .item_response;

        item_ui::select_hovered_on_click(ctx, &response, &[item]);

        if remove {
            self.blueprint.mark_user_interaction(ctx);
            self.deferred_tree_actions.remove.push(tile_id);
        }

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Container visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            self.tree.set_visible(tile_id, visible);
        }
    }

    fn space_view_entry_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        space_view_id: &SpaceViewId,
    ) {
        let Some(space_view) = self.blueprint.space_views.get(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            self.deferred_tree_actions.remove.push(tile_id);
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        // TODO(jleibs): Sort out borrow-checker to avoid the need to clone here
        // while still being able to pass &ViewerContext down the chain.
        let query_result = ctx.lookup_query_result(space_view.query_id()).clone();

        let result_tree = &query_result.tree;

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let visible_child = visible;
        let item = Item::SpaceView(space_view.id);

        let default_open = result_tree
            .root_node()
            .map_or(false, Self::default_open_for_data_result);

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
                if let Some(result_handle) = result_tree.root_handle() {
                    // TODO(jleibs): handle the case where the only result
                    // in the tree is a single path (no groups). This should never
                    // happen for a SpaceViewContents.
                    Self::space_view_blueprint_ui(
                        ctx,
                        ui,
                        &query_result,
                        result_handle,
                        space_view,
                        visible_child,
                    );
                } else {
                    ui.label("No results");
                }
            })
            .item_response
            .on_hover_text("Space View");

        if response.clicked() {
            self.deferred_tree_actions.focus_tab = Some(space_view.id);
        }

        item_ui::select_hovered_on_click(ctx, &response, &[item]);

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Space view visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            self.tree.set_visible(tile_id, visible);
        }
    }

    fn space_view_blueprint_ui(
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        query_result: &DataQueryResult,
        result_handle: DataResultHandle,
        space_view: &SpaceViewBlueprint,
        space_view_visible: bool,
    ) {
        let Some(top_node) = query_result.tree.lookup_node(result_handle) else {
            debug_assert!(false, "Invalid data result handle in data result tree");
            return;
        };

        let group_is_visible =
            top_node.data_result.resolved_properties.visible && space_view_visible;

        // Always real children ahead of groups
        for child in top_node
            .children
            .iter()
            .filter(|c| {
                query_result
                    .tree
                    .lookup_result(**c)
                    .map_or(false, |c| !c.is_group)
            })
            .chain(top_node.children.iter().filter(|c| {
                query_result
                    .tree
                    .lookup_result(**c)
                    .map_or(false, |c| c.is_group)
            }))
        {
            let Some(child_node) = query_result.tree.lookup_node(*child) else {
                debug_assert!(false, "DataResultNode {top_node:?} has an invalid child");
                continue;
            };

            let data_result = &child_node.data_result;
            let entity_path = &child_node.data_result.entity_path;

            let item = if data_result.is_group {
                // If we can't find a group_handle for some reason, use the default, null handle.
                Item::DataBlueprintGroup(space_view.id, query_result.id, entity_path.clone())
            } else {
                Item::InstancePath(
                    Some(space_view.id),
                    InstancePath::entity_splat(entity_path.clone()),
                )
            };

            let is_selected = ctx.selection().contains(&item);

            let is_item_hovered =
                ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

            let mut properties = data_result
                .individual_properties
                .clone()
                .unwrap_or_default();

            let name = entity_path
                .iter()
                .last()
                .map_or("unknown".to_owned(), |e| e.to_string());

            let response = if child_node.children.is_empty() {
                let label = format!("🔹 {name}");

                ListItem::new(ctx.re_ui, label)
                    .selected(is_selected)
                    .subdued(
                        !group_is_visible
                            || !properties.visible
                            || data_result.view_parts.is_empty(),
                    )
                    .force_hovered(is_item_hovered)
                    .with_buttons(|re_ui, ui| {
                        let vis_response = visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut properties.visible,
                        );

                        let response =
                            remove_button_ui(re_ui, ui, "Remove Entity from the Space View");
                        if response.clicked() {
                            space_view.add_entity_exclusion(
                                ctx,
                                EntityPathExpr::Exact(entity_path.clone()),
                            );
                        }

                        response | vis_response
                    })
                    .show(ui)
                    .on_hover_ui(|ui| {
                        if data_result.is_group {
                            ui.label("Group");
                        } else {
                            re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, entity_path);
                        }
                    })
            } else {
                let default_open = Self::default_open_for_data_result(child_node);
                let mut remove_group = false;

                let response = ListItem::new(ctx.re_ui, name)
                    .selected(is_selected)
                    .subdued(!properties.visible || !group_is_visible)
                    .force_hovered(is_item_hovered)
                    .with_icon(&re_ui::icons::CONTAINER)
                    .with_buttons(|re_ui, ui| {
                        let vis_response = visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut properties.visible,
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
                    .show_collapsing(ui, ui.id().with(child), default_open, |_, ui| {
                        Self::space_view_blueprint_ui(
                            ctx,
                            ui,
                            query_result,
                            *child,
                            space_view,
                            space_view_visible,
                        );
                    })
                    .item_response
                    .on_hover_ui(|ui| {
                        if data_result.is_group {
                            ui.label("Group");
                        } else {
                            re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, entity_path);
                        }
                    });

                if remove_group {
                    space_view
                        .add_entity_exclusion(ctx, EntityPathExpr::Recursive(entity_path.clone()));
                }

                response
            };
            data_result.save_override(Some(properties), ctx);

            item_ui::select_hovered_on_click(ctx, &response, &[item]);
        }
    }

    pub fn add_new_spaceview_button_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        ui.menu_image_button(
            re_ui::icons::ADD
                .as_image()
                .fit_to_exact_size(re_ui::ReUi::small_icon_size()),
            |ui| {
                ui.style_mut().wrap = Some(false);

                let mut add_space_view_item =
                    |ui: &mut egui::Ui, space_view: SpaceViewBlueprint| {
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
                            ctx.set_single_selection(&Item::SpaceView(space_view.id));
                            self.blueprint.add_space_views(
                                std::iter::once(space_view),
                                ctx,
                                &mut self.deferred_tree_actions,
                            );
                        }
                    };

                // Space view options proposed by heuristics
                let mut possible_space_views =
                    all_possible_space_views(ctx, spaces_info, ctx.entities_per_system_per_class);
                possible_space_views
                    .sort_by_key(|(space_view, _)| space_view.space_origin.to_string());

                let has_possible_space_views = !possible_space_views.is_empty();
                for (space_view, _) in possible_space_views {
                    add_space_view_item(ui, space_view);
                }

                if has_possible_space_views {
                    ui.separator();
                }

                // Empty space views of every available types
                for space_view in ctx
                    .space_view_class_registry
                    .iter_classes()
                    .sorted_by_key(|space_view_class| space_view_class.display_name())
                    .map(|class| {
                        SpaceViewBlueprint::new(
                            class.identifier(),
                            &format!("empty {}", class.display_name()),
                            &EntityPath::root(),
                            DataQueryBlueprint::new(class.identifier(), std::iter::empty()),
                        )
                    })
                {
                    add_space_view_item(ui, space_view);
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
