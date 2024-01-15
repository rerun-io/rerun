use egui::{Response, Ui};
use itertools::Itertools;

use re_data_ui::item_ui;
use re_entity_db::InstancePath;
use re_log_types::{EntityPath, EntityPathRule};
use re_space_view::DataQueryBlueprint;
use re_ui::list_item::ListItem;
use re_ui::ReUi;
use re_viewer_context::{
    DataQueryResult, DataResultNode, HoverHighlight, Item, SpaceViewId, ViewerContext,
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
                        self.tile_ui(ctx, ui, root, true);
                    }
                });
            });
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_data_result(group: &DataResultNode) -> bool {
        let num_children = group.children.len();
        2 <= num_children && num_children <= 3
    }

    fn tile_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        parent_visible: bool,
    ) {
        // Temporarily remove the tile so we don't get borrow-checker fights:
        let Some(mut tile) = self.tree.tiles.remove(tile_id) else {
            return;
        };

        match &mut tile {
            egui_tiles::Tile::Container(container) => {
                self.container_tree_ui(ctx, ui, tile_id, container, parent_visible);
            }
            egui_tiles::Tile::Pane(space_view_id) => {
                // A space view
                self.space_view_entry_ui(ctx, ui, tile_id, space_view_id, parent_visible);
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
        parent_visible: bool,
    ) {
        // TODO(#4285): this will disappear once we walk the container blueprint tree instead of `egui_tiles::Tree`
        if let (egui_tiles::Container::Tabs(_), Some(child_id)) =
            (container, container.only_child())
        {
            // Maybe a tab container with only one child - collapse it in the tree view to make it more easily understood.
            // This means we won't be showing the visibility button of the parent container,
            // so if the child is made invisible, we should do the same for the parent.
            let child_is_visible = self.tree.is_visible(child_id);

            let visible = self.tree.is_visible(tile_id);

            if visible != child_is_visible {
                self.tree.set_visible(tile_id, child_is_visible);
                // TODO(#4687): Be extra careful here. If we mark edited inappropriately we can create an infinite edit loop.
                self.edited = true;
            }

            return self.tile_ui(ctx, ui, child_id, parent_visible);
        }

        let item = Item::Container(tile_id);

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let container_visible = visible && parent_visible;
        let mut remove = false;

        let default_open = true;

        let response = ListItem::new(ctx.re_ui, format!("{:?}", container.kind()))
            .subdued(!container_visible)
            .selected(ctx.selection().contains_item(&item))
            .with_icon(crate::icon_for_container_kind(&container.kind()))
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, parent_visible, &mut visible);
                visibility_changed = vis_response.changed();

                let remove_response = remove_button_ui(re_ui, ui, "Remove container");
                remove = remove_response.clicked();

                remove_response | vis_response
            })
            .show_collapsing(ui, ui.id().with(tile_id), default_open, |_, ui| {
                for &child in container.children() {
                    self.tile_ui(ctx, ui, child, container_visible);
                }
            })
            .item_response;

        item_ui::select_hovered_on_click(ctx, &response, item);

        if remove {
            self.blueprint.mark_user_interaction(ctx);
            self.blueprint.remove(tile_id);
        }

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Container visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            self.tree.set_visible(tile_id, visible);
            // TODO(#4687): Be extra careful here. If we mark edited inappropriately we can create an infinite edit loop.
            self.edited = true;
        }
    }

    fn space_view_entry_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        space_view_id: &SpaceViewId,
        container_visible: bool,
    ) {
        let Some(space_view) = self.blueprint.space_views.get(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            self.blueprint.remove(tile_id);
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        // TODO(jleibs): Sort out borrow-checker to avoid the need to clone here
        // while still being able to pass &ViewerContext down the chain.
        let query_result = ctx.lookup_query_result(space_view.query_id()).clone();

        let result_tree = &query_result.tree;

        let mut visibility_changed = false;
        let mut visible = self.tree.is_visible(tile_id);
        let space_view_visible = visible && container_visible;
        let item = Item::SpaceView(space_view.id);

        let root_node = result_tree.first_interesting_root();

        let default_open = root_node.map_or(false, Self::default_open_for_data_result);

        let collapsing_header_id = ui.id().with(space_view.id);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let (label, named) = space_view.display_name_or_default();

        let response = ListItem::new(ctx.re_ui, label)
            .unnamed_style(!named)
            .with_icon(space_view.class(ctx.space_view_class_registry).icon())
            .selected(ctx.selection().contains_item(&item))
            .subdued(!space_view_visible)
            .force_hovered(is_item_hovered)
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, container_visible, &mut visible);
                visibility_changed = vis_response.changed();

                let response = remove_button_ui(re_ui, ui, "Remove Space View from the Viewport");
                if response.clicked() {
                    self.blueprint.remove(tile_id);
                }

                response | vis_response
            })
            .show_collapsing(ui, collapsing_header_id, default_open, |_, ui| {
                if let Some(result_node) = root_node {
                    // TODO(jleibs): handle the case where the only result
                    // in the tree is a single path (no groups). This should never
                    // happen for a SpaceViewContents.
                    Self::space_view_blueprint_ui(
                        ctx,
                        ui,
                        &query_result,
                        result_node,
                        space_view,
                        space_view_visible,
                    );
                } else {
                    ui.label("No results");
                }
            })
            .item_response
            .on_hover_text("Space View");

        if response.clicked() {
            self.blueprint.focus_tab(space_view.id);
        }

        item_ui::select_hovered_on_click(ctx, &response, item);

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Space view visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            if ctx.app_options.legacy_container_blueprint {
                self.tree.set_visible(tile_id, visible);
            } else {
                // Note: we set visibility directly on the space view so it gets saved
                // to the blueprint directly. If we set it on the tree there are some
                // edge-cases where visibility can get lost when we simplify out trivial
                // tab-containers.
                space_view.set_visible(visible, ctx);
            }
        }
    }

    fn space_view_blueprint_ui(
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        query_result: &DataQueryResult,
        top_node: &DataResultNode,
        space_view: &SpaceViewBlueprint,
        space_view_visible: bool,
    ) {
        let group_is_visible =
            top_node.data_result.accumulated_properties().visible && space_view_visible;

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

            let is_selected = ctx.selection().contains_item(&item);

            let is_item_hovered =
                ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

            let mut properties = data_result
                .individual_properties()
                .cloned()
                .unwrap_or_default();

            let name = entity_path
                .iter()
                .last()
                .map_or("unknown".to_owned(), |e| e.ui_string());

            let response = if child_node.children.is_empty() {
                let label = format!("🔹 {name}");

                ListItem::new(ctx.re_ui, label)
                    .selected(is_selected)
                    .subdued(
                        !group_is_visible
                            || !properties.visible
                            || data_result.visualizers.is_empty(),
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
                                EntityPathRule::exact(entity_path.clone()),
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
                    .show_collapsing(
                        ui,
                        ui.id().with(&child_node.data_result.entity_path),
                        default_open,
                        |_, ui| {
                            Self::space_view_blueprint_ui(
                                ctx,
                                ui,
                                query_result,
                                child_node,
                                space_view,
                                space_view_visible,
                            );
                        },
                    )
                    .item_response
                    .on_hover_ui(|ui| {
                        if data_result.is_group {
                            ui.label("Group");
                        } else {
                            re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, entity_path);
                        }
                    });

                if remove_group {
                    space_view.add_entity_exclusion(
                        ctx,
                        EntityPathRule::including_subtree(entity_path.clone()),
                    );
                }

                response
            };
            data_result.save_override(Some(properties), ctx);

            item_ui::select_hovered_on_click(ctx, &response, item);
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

                let add_space_view_item =
                    |ui: &mut egui::Ui, space_view: SpaceViewBlueprint, empty: bool| {
                        let label = if empty {
                            format!(
                                "Empty {} view",
                                space_view
                                    .class(ctx.space_view_class_registry)
                                    .display_name()
                            )
                        } else {
                            format!(
                                "{} view of {}",
                                space_view
                                    .class(ctx.space_view_class_registry)
                                    .display_name(),
                                space_view.space_origin
                            )
                        };

                        if ctx
                            .re_ui
                            .selectable_label_with_icon(
                                ui,
                                space_view.class(ctx.space_view_class_registry).icon(),
                                label,
                                false,
                                re_ui::LabelStyle::Normal,
                            )
                            .clicked()
                        {
                            ui.close_menu();
                            ctx.selection_state()
                                .set_selection(Item::SpaceView(space_view.id));

                            let new_ids = self.blueprint.add_space_views(
                                std::iter::once(space_view),
                                ctx,
                                None, //TODO(ab): maybe add to the currently selected container instead?
                            );
                            if let Some(new_id) = new_ids.first() {
                                self.blueprint.focus_tab(*new_id);
                            }
                        }
                    };

                // Space view options proposed by heuristics
                let mut possible_space_views = all_possible_space_views(ctx, spaces_info);
                possible_space_views
                    .sort_by_key(|(space_view, _)| space_view.space_origin.to_string());

                let has_possible_space_views = !possible_space_views.is_empty();
                for (space_view, _) in possible_space_views {
                    add_space_view_item(ui, space_view, false);
                }

                if has_possible_space_views {
                    ui.separator();
                }

                // Empty space views of every available types
                for space_view in ctx
                    .space_view_class_registry
                    .iter_registry()
                    .sorted_by_key(|entry| entry.class.display_name())
                    .map(|entry| {
                        SpaceViewBlueprint::new(
                            entry.class.identifier(),
                            &EntityPath::root(),
                            DataQueryBlueprint::new(entry.class.identifier(), Default::default()),
                        )
                    })
                {
                    add_space_view_item(ui, space_view, true);
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
