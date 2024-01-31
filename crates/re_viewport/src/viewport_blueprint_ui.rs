use egui::{Response, Ui};
use itertools::Itertools;

use re_entity_db::InstancePath;
use re_log_types::{EntityPath, EntityPathRule};
use re_space_view::DataQueryBlueprint;
use re_ui::{drag_and_drop::DropTarget, list_item::ListItem, ReUi};
use re_viewer_context::{
    ContainerId, DataQueryResult, DataResultNode, HoverHighlight, Item, SpaceViewId, ViewerContext,
};

use crate::{
    container::Contents, space_view_heuristics::default_created_space_views, SpaceViewBlueprint,
    Viewport,
};

impl Viewport<'_, '_> {
    /// Show the blueprint panel tree view.
    pub fn tree_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .id_source("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                ctx.re_ui.panel_content(ui, |_, ui| {
                    if let Some(root_container) = self.blueprint.root_container {
                        self.contents_ui(ctx, ui, &Contents::Container(root_container), true);
                    }

                    let empty_space_response =
                        ui.allocate_response(ui.available_size(), egui::Sense::click());

                    // clear selection upon clicking on empty space
                    if empty_space_response.clicked() {
                        ctx.selection_state().clear_current();
                    }

                    // handle drag and drop interaction on empty space
                    self.handle_empty_space_drag_and_drop_interaction(
                        ui,
                        empty_space_response.rect,
                    );
                });
            });
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_data_result(group: &DataResultNode) -> bool {
        let num_children = group.children.len();
        2 <= num_children && num_children <= 3
    }

    fn contents_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        contents: &Contents,
        parent_visible: bool,
    ) {
        match contents {
            Contents::Container(container_id) => {
                self.container_tree_ui(ctx, ui, container_id, parent_visible);
            }
            Contents::SpaceView(space_view_id) => {
                self.space_view_entry_ui(ctx, ui, space_view_id, parent_visible);
            }
        };
    }

    fn container_tree_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        container_id: &ContainerId,
        parent_visible: bool,
    ) {
        let item = Item::Container(*container_id);

        let Some(container_blueprint) = self.blueprint.containers.get(container_id) else {
            re_log::warn_once!("Ignoring unknown container {container_id}");
            return;
        };

        let mut visibility_changed = false;
        let mut visible = container_blueprint.visible;
        let container_visible = visible && parent_visible;
        let mut remove = false;

        let default_open = true;

        let re_ui::list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
        } = ListItem::new(
            ctx.re_ui,
            format!("{:?}", container_blueprint.container_kind),
        )
        .subdued(!container_visible)
        .selected(ctx.selection().contains_item(&item))
        .draggable(true)
        .drop_target_style(self.state.is_candidate_drop_parent_container(container_id))
        .label_style(re_ui::LabelStyle::Unnamed)
        .with_icon(crate::icon_for_container_kind(
            &container_blueprint.container_kind,
        ))
        .with_buttons(|re_ui, ui| {
            let vis_response = visibility_button_ui(re_ui, ui, parent_visible, &mut visible);
            visibility_changed = vis_response.changed();

            let remove_response = remove_button_ui(re_ui, ui, "Remove container");
            remove = remove_response.clicked();

            remove_response | vis_response
        })
        .show_collapsing(ui, ui.id().with(container_id), default_open, |_, ui| {
            for child in &container_blueprint.contents {
                self.contents_ui(ctx, ui, child, container_visible);
            }
        });

        ctx.select_hovered_on_click(&response, item);

        self.handle_drag_and_drop_interaction(
            ctx,
            ui,
            Contents::Container(*container_id),
            &response,
            body_response.as_ref().map(|r| &r.response),
        );

        if remove {
            self.blueprint.mark_user_interaction(ctx);
            self.blueprint
                .remove_contents(Contents::Container(*container_id));
        }

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Container visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            container_blueprint.set_visible(ctx, visible);
        }
    }

    fn space_view_entry_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view_id: &SpaceViewId,
        container_visible: bool,
    ) {
        let Some(space_view) = self.blueprint.space_views.get(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        // TODO(jleibs): Sort out borrow-checker to avoid the need to clone here
        // while still being able to pass &ViewerContext down the chain.
        let query_result = ctx.lookup_query_result(space_view.query_id()).clone();

        let result_tree = &query_result.tree;

        let mut visibility_changed = false;
        let mut visible = space_view.visible;
        let space_view_visible = visible && container_visible;
        let item = Item::SpaceView(space_view.id);

        let root_node = result_tree.first_interesting_root();

        // empty space views should display as open by default to highlight the fact that they are empty
        let default_open = root_node.map_or(true, Self::default_open_for_data_result);

        let collapsing_header_id = ui.id().with(space_view.id);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let space_view_name = space_view.display_name_or_default();

        let re_ui::list_item::ShowCollapsingResponse {
            item_response: mut response,
            body_response,
        } = ListItem::new(ctx.re_ui, space_view_name.as_ref())
            .label_style(space_view_name.style())
            .with_icon(space_view.class(ctx.space_view_class_registry).icon())
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .subdued(!space_view_visible)
            .force_hovered(is_item_hovered)
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, container_visible, &mut visible);
                visibility_changed = vis_response.changed();

                let response = remove_button_ui(re_ui, ui, "Remove Space View from the Viewport");
                if response.clicked() {
                    self.blueprint.mark_user_interaction(ctx);
                    self.blueprint
                        .remove_contents(Contents::SpaceView(*space_view_id));
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
                    ui.label("No data");
                }
            });

        response = response.on_hover_text("Space View");

        if response.clicked() {
            self.blueprint.focus_tab(space_view.id);
        }

        ctx.select_hovered_on_click(&response, item);

        self.handle_drag_and_drop_interaction(
            ctx,
            ui,
            Contents::SpaceView(*space_view_id),
            &response,
            body_response.as_ref().map(|r| &r.response),
        );

        if visibility_changed {
            if self.blueprint.auto_layout {
                re_log::trace!("Space view visibility changed - will no longer auto-layout");
            }

            // Keep `auto_space_views` enabled.
            self.blueprint.set_auto_layout(false, ctx);

            // Note: we set visibility directly on the space view so it gets saved
            // to the blueprint directly. If we set it on the tree there are some
            // edge-cases where visibility can get lost when we simplify out trivial
            // tab-containers.
            space_view.set_visible(ctx, visible);
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
        let query = ctx.current_query();
        let store = ctx.entity_db.store();

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
                            re_data_ui::item_ui::entity_hover_card_ui(
                                ui,
                                ctx,
                                &query,
                                store,
                                entity_path,
                            );
                        }
                    })
            } else {
                let default_open = Self::default_open_for_data_result(child_node);
                let mut remove_group = false;

                let response = ListItem::new(ctx.re_ui, name)
                    .selected(is_selected)
                    .subdued(!properties.visible || !group_is_visible)
                    .force_hovered(is_item_hovered)
                    .with_icon(&re_ui::icons::GROUP)
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
                            re_data_ui::item_ui::entity_hover_card_ui(
                                ui,
                                ctx,
                                &query,
                                store,
                                entity_path,
                            );
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

            ctx.select_hovered_on_click(&response, item);
        }
    }

    pub fn add_new_spaceview_button_ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.menu_image_button(
            re_ui::icons::ADD
                .as_image()
                .fit_to_exact_size(re_ui::ReUi::small_icon_size()),
            |ui| {
                ui.style_mut().wrap = Some(false);

                let add_space_view_item =
                    |ui: &mut egui::Ui, space_view: SpaceViewBlueprint, empty: bool| {
                        let display_name = space_view
                            .class(ctx.space_view_class_registry)
                            .display_name();
                        let label = if empty {
                            format!("Empty {display_name} view",)
                        } else {
                            format!("{display_name} view of {}", space_view.space_origin)
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
                let mut possible_space_views = default_created_space_views(ctx);
                possible_space_views.sort_by_key(|space_view| space_view.space_origin.to_string());

                let has_possible_space_views = !possible_space_views.is_empty();
                for space_view in possible_space_views {
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

    // ----------------------------------------------------------------------------
    // drag and drop support

    fn handle_drag_and_drop_interaction(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &egui::Ui,
        contents: Contents,
        response: &egui::Response,
        body_response: Option<&egui::Response>,
    ) {
        //
        // initiate drag and force single-selection
        //

        if response.drag_started() {
            ctx.selection_state().set_selection(contents.as_item());
            egui::DragAndDrop::set_payload(ui.ctx(), contents);
        }

        //
        // check if a drag is in progress and set the cursor accordingly
        //

        let Some(dragged_item_id) = egui::DragAndDrop::payload(ui.ctx()).map(|payload| *payload)
        else {
            // nothing is being dragged, so nothing to do
            return;
        };

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        //
        // find our parent, our position within parent, and the previous container (if any)
        //

        let Some((parent_container_id, pos_in_parent)) =
            self.blueprint.find_parent_and_position_index(&contents)
        else {
            return;
        };

        let previous_container = if pos_in_parent > 0 {
            self.blueprint
                .container(&parent_container_id)
                .map(|container| container.contents[pos_in_parent - 1])
                .filter(|contents| matches!(contents, Contents::Container(_)))
        } else {
            None
        };

        //
        // find the drop target
        //

        // Prepare the item description structure needed by `find_drop_target`. Here, we use
        // `Contents` for the "ItemId" generic type parameter.
        let item_desc = re_ui::drag_and_drop::ItemContext {
            id: contents,
            is_container: matches!(contents, Contents::Container(_)),
            parent_id: Contents::Container(parent_container_id),
            position_index_in_parent: pos_in_parent,
            previous_container_id: previous_container,
        };

        let drop_target = re_ui::drag_and_drop::find_drop_target(
            ui,
            &item_desc,
            response.rect,
            body_response.map(|r| r.rect),
            ReUi::list_item_height(),
        );

        if let Some(drop_target) = drop_target {
            self.handle_drop_target(ui, dragged_item_id, &drop_target);
        }
    }

    fn handle_empty_space_drag_and_drop_interaction(&self, ui: &egui::Ui, empty_space: egui::Rect) {
        //
        // check if a drag is in progress and set the cursor accordingly
        //

        let Some(dragged_item_id) = egui::DragAndDrop::payload(ui.ctx()).map(|payload| *payload)
        else {
            // nothing is being dragged, so nothing to do
            return;
        };

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        //
        // prepare a drop target corresponding to "insert last in root container"
        //
        // TODO(ab): this is a rather primitive behavior. Ideally we should allow dropping in the last container based
        //           on the horizontal position of the cursor.

        let Some(root_container_id) = self.blueprint.root_container else {
            return;
        };

        if ui.rect_contains_pointer(empty_space) {
            let drop_target = re_ui::drag_and_drop::DropTarget::new(
                // TODO(#4909): this indent is a visual hack that should be remove once #4909 is done
                (empty_space.left() + ui.spacing().indent..=empty_space.right()).into(),
                empty_space.top(),
                Contents::Container(root_container_id),
                usize::MAX,
            );

            self.handle_drop_target(ui, dragged_item_id, &drop_target);
        }
    }

    fn handle_drop_target(
        &self,
        ui: &Ui,
        dragged_item_id: Contents,
        drop_target: &DropTarget<Contents>,
    ) {
        // We cannot allow the target location to be "inside" the dragged item, because that would amount moving
        // myself inside of me.
        if let Contents::Container(dragged_container_id) = &dragged_item_id {
            if self
                .blueprint
                .is_contents_in_container(&drop_target.target_parent_id, dragged_container_id)
            {
                return;
            }
        }

        ui.painter().hline(
            drop_target.indicator_span_x,
            drop_target.indicator_position_y,
            (2.0, egui::Color32::WHITE),
        );

        let Contents::Container(target_container_id) = drop_target.target_parent_id else {
            // this shouldn't append
            return;
        };

        if ui.input(|i| i.pointer.any_released()) {
            self.blueprint.move_contents(
                dragged_item_id,
                target_container_id,
                drop_target.target_position_index,
            );

            egui::DragAndDrop::clear_payload(ui.ctx());
        } else {
            self.blueprint.set_drop_target(&target_container_id);
        }
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
