use egui::{Response, Ui};
use itertools::Itertools;
use re_data_ui::item_ui::guess_instance_path_icon;
use smallvec::SmallVec;

use re_context_menu::{context_menu_ui_for_item, SelectionUpdateBehavior};
use re_entity_db::InstancePath;
use re_log_types::EntityPath;
use re_types::blueprint::components::Visible;
use re_ui::{drag_and_drop::DropTarget, list_item, ContextExt as _, DesignTokens, UiExt as _};
use re_viewer_context::{
    contents_name_style, icon_for_container_kind, CollapseScope, Contents, DataResultNodeOrPath,
    DragAndDropFeedback, DragAndDropPayload, SystemCommandSender,
};
use re_viewer_context::{
    ContainerId, DataQueryResult, DataResultNode, HoverHighlight, Item, ViewId, ViewerContext,
};
use re_viewport_blueprint::ui::show_add_view_or_container_modal;
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

/// Holds the state of the blueprint tree UI.
#[derive(Default)]
pub struct BlueprintTree {
    /// The item that should be focused on in the blueprint tree.
    ///
    /// Set at each frame by [`Self::tree_ui`]. This is similar to
    /// [`ViewerContext::focused_item`] but account for how specifically the blueprint tree should
    /// handle the focused item.
    blueprint_tree_scroll_to_item: Option<Item>,

    /// Current candidate parent container for the ongoing drop. Should be drawn with special
    /// highlight.
    ///
    /// See [`Self::is_candidate_drop_parent_container`] for details.
    candidate_drop_parent_container_id: Option<ContainerId>,

    /// Candidate parent container to be drawn on next frame.
    ///
    /// We double-buffer this value to deal with ordering constraints.
    next_candidate_drop_parent_container_id: Option<ContainerId>,
}

impl BlueprintTree {
    /// Show the Blueprint section of the left panel based on the current [`ViewportBlueprint`]
    pub fn show(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) {
        ui.panel_content(|ui| {
            ui.panel_title_bar_with_buttons(
                "Blueprint",
                Some("The blueprint is where you can configure the Rerun Viewer"),
                |ui| {
                    self.add_new_view_button_ui(ctx, viewport, ui);
                    reset_blueprint_button_ui(ctx, ui);
                },
            );
        });

        // This call is excluded from `panel_content` because it has a ScrollArea, which should not be
        // inset. Instead, it calls panel_content itself inside the ScrollArea.
        self.tree_ui(ctx, viewport, ui);
    }

    /// Show the blueprint panel tree view.
    fn tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) {
        re_tracing::profile_function!();

        // The candidate drop parent container is double-buffered, so here we have the buffer swap.
        self.candidate_drop_parent_container_id = self.next_candidate_drop_parent_container_id;
        self.next_candidate_drop_parent_container_id = None;

        egui::ScrollArea::both()
            .id_salt("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                ui.panel_content(|ui| {
                    self.blueprint_tree_scroll_to_item = ctx
                        .focused_item
                        .as_ref()
                        .and_then(|item| handle_focused_item(ctx, viewport, ui, item));

                    list_item::list_item_scope(ui, "blueprint tree", |ui| {
                        self.root_container_tree_ui(ctx, viewport, ui);
                    });

                    let empty_space_response =
                        ui.allocate_response(ui.available_size(), egui::Sense::click());

                    // clear selection upon clicking on empty space
                    if empty_space_response.clicked() {
                        ctx.selection_state().clear_selection();
                    }

                    // handle drag and drop interaction on empty space
                    self.handle_empty_space_drag_and_drop_interaction(
                        ctx,
                        viewport,
                        ui,
                        empty_space_response.rect,
                    );
                });
            });
    }

    /// Check if the provided item should be scrolled to.
    fn scroll_to_me_if_needed(&self, ui: &egui::Ui, item: &Item, response: &egui::Response) {
        if Some(item) == self.blueprint_tree_scroll_to_item.as_ref() {
            // Scroll only if the entity isn't already visible. This is important because that's what
            // happens when double-clicking an entity _in the blueprint tree_. In such case, it would be
            // annoying to induce a scroll motion.
            if !ui.clip_rect().contains_rect(response.rect) {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }
    }

    /// If a group or view has a total of this number of elements, show its subtree by default?
    fn default_open_for_data_result(group: &DataResultNode) -> bool {
        let num_children = group.children.len();
        2 <= num_children && num_children <= 3
    }

    fn contents_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
        contents: &Contents,
        parent_visible: bool,
    ) {
        match contents {
            Contents::Container(container_id) => {
                self.container_tree_ui(ctx, viewport, ui, container_id, parent_visible);
            }
            Contents::View(view_id) => {
                self.view_entry_ui(ctx, viewport, ui, view_id, parent_visible);
            }
        };
    }

    /// Display the root container.
    ///
    /// The root container is different from other containers in that it cannot be removed or dragged, and it cannot be
    /// collapsed, so it's drawn without a collapsing triangle.
    fn root_container_tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) {
        let container_id = viewport.root_container;

        let Some(container_blueprint) = viewport.container(&container_id) else {
            re_log::warn!("Failed to find root container {container_id} in ViewportBlueprint");
            return;
        };

        let item = Item::Container(container_id);
        let container_name = container_blueprint.display_name_or_default();

        let item_response = ui
            .list_item()
            .selected(ctx.selection().contains_item(&item))
            .draggable(true) // allowed for consistency but results in an invalid drag
            .drop_target_style(self.is_candidate_drop_parent_container(&container_id))
            .show_flat(
                ui,
                list_item::LabelContent::new(format!("Viewport ({})", container_name.as_ref()))
                    .label_style(contents_name_style(&container_name))
                    .with_icon(icon_for_container_kind(&container_blueprint.container_kind)),
            );

        for child in &container_blueprint.contents {
            self.contents_ui(ctx, viewport, ui, child, true);
        }

        context_menu_ui_for_item(
            ctx,
            viewport,
            &item,
            &item_response,
            SelectionUpdateBehavior::UseSelection,
        );
        self.scroll_to_me_if_needed(ui, &item, &item_response);
        ctx.handle_select_hover_drag_interactions(&item_response, item, true);

        self.handle_root_container_drag_and_drop_interaction(
            ctx,
            viewport,
            ui,
            Contents::Container(container_id),
            &item_response,
        );
    }

    fn container_tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
        container_id: &ContainerId,
        parent_visible: bool,
    ) {
        let item = Item::Container(*container_id);
        let content = Contents::Container(*container_id);

        let Some(container_blueprint) = viewport.container(container_id) else {
            re_log::warn_once!("Ignoring unknown container {container_id}");
            return;
        };

        let mut visible = container_blueprint.visible;
        let container_visible = visible && parent_visible;

        let default_open = true;

        let container_name = container_blueprint.display_name_or_default();

        let item_content = list_item::LabelContent::new(container_name.as_ref())
            .subdued(!container_visible)
            .label_style(contents_name_style(&container_name))
            .with_icon(icon_for_container_kind(&container_blueprint.container_kind))
            .with_buttons(|ui| {
                let vis_response = visibility_button_ui(ui, parent_visible, &mut visible);

                let remove_response = remove_button_ui(ui, "Remove container");
                if remove_response.clicked() {
                    viewport.mark_user_interaction(ctx);
                    viewport.remove_contents(content);
                }

                remove_response | vis_response
            });

        // Globally unique id - should only be one of these in view at one time.
        // We do this so that we can support "collapse/expand all" command.
        let id = egui::Id::new(CollapseScope::BlueprintTree.container(*container_id));

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .drop_target_style(self.is_candidate_drop_parent_container(container_id))
            .show_hierarchical_with_children(ui, id, default_open, item_content, |ui| {
                for child in &container_blueprint.contents {
                    self.contents_ui(ctx, viewport, ui, child, container_visible);
                }
            });

        let response = response.on_hover_text(format!(
            "{:?} container",
            container_blueprint.container_kind
        ));

        context_menu_ui_for_item(
            ctx,
            viewport,
            &item,
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        self.scroll_to_me_if_needed(ui, &item, &response);
        ctx.handle_select_hover_drag_interactions(&response, item, true);

        viewport.set_content_visibility(ctx, &content, visible);

        self.handle_drag_and_drop_interaction(
            ctx,
            viewport,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    fn view_entry_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
        view_id: &ViewId,
        container_visible: bool,
    ) {
        let Some(view) = viewport.view(view_id) else {
            re_log::warn_once!("Bug: asked to show a UI for a view that doesn't exist");
            return;
        };
        debug_assert_eq!(view.id, *view_id);

        let query_result = ctx.lookup_query_result(view.id);
        let result_tree = &query_result.tree;

        let mut visible = view.visible;
        let view_visible = visible && container_visible;
        let item = Item::View(view.id);

        let root_node = result_tree.root_node();

        // empty views should display as open by default to highlight the fact that they are empty
        let default_open = root_node.map_or(true, Self::default_open_for_data_result);

        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let class = &view.class(ctx.view_class_registry);
        let view_name = view.display_name_or_default();

        let item_content = list_item::LabelContent::new(view_name.as_ref())
            .label_style(contents_name_style(&view_name))
            .with_icon(class.icon())
            .subdued(!view_visible)
            .with_buttons(|ui| {
                let vis_response = visibility_button_ui(ui, container_visible, &mut visible);

                let response = remove_button_ui(ui, "Remove view from the viewport");
                if response.clicked() {
                    viewport.mark_user_interaction(ctx);
                    viewport.remove_contents(Contents::View(*view_id));
                }

                response | vis_response
            });

        // Globally unique id - should only be one of these in view at one time.
        // We do this so that we can support "collapse/expand all" command.
        let id = egui::Id::new(CollapseScope::BlueprintTree.view(*view_id));

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .force_hovered(is_item_hovered)
            .show_hierarchical_with_children(ui, id, default_open, item_content, |ui| {
                // Always show the origin hierarchy first.
                self.view_entity_hierarchy_ui(
                    ctx,
                    viewport,
                    ui,
                    query_result,
                    &DataResultNodeOrPath::from_path_lookup(result_tree, &view.space_origin),
                    view,
                    view_visible,
                    false,
                );

                // Show 'projections' if there's any items that weren't part of the tree under origin but are directly included.
                // The latter is important since `+ image/camera/**` necessarily has `image` and `image/camera` in the data result tree.
                let mut projections = Vec::new();
                result_tree.visit(&mut |node| {
                    if node.data_result.entity_path.starts_with(&view.space_origin) {
                        false // If it's under the origin, we're not interested, stop recursing.
                    } else if node.data_result.tree_prefix_only {
                        true // Keep recursing until we find a projection.
                    } else {
                        projections.push(node);
                        false // We found a projection, stop recursing as everything below is now included in the projections.
                    }
                });
                if !projections.is_empty() {
                    ui.list_item().interactive(false).show_flat(
                        ui,
                        list_item::LabelContent::new("Projections:").italics(true),
                    );

                    for projection in projections {
                        self.view_entity_hierarchy_ui(
                            ctx,
                            viewport,
                            ui,
                            query_result,
                            &DataResultNodeOrPath::DataResultNode(projection),
                            view,
                            view_visible,
                            true,
                        );
                    }
                }
            });

        let response = response.on_hover_text(format!("{} view", class.display_name()));

        if response.clicked() {
            viewport.focus_tab(view.id);
        }

        context_menu_ui_for_item(
            ctx,
            viewport,
            &item,
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        self.scroll_to_me_if_needed(ui, &item, &response);
        ctx.handle_select_hover_drag_interactions(&response, item, true);

        let content = Contents::View(*view_id);

        viewport.set_content_visibility(ctx, &content, visible);
        self.handle_drag_and_drop_interaction(
            ctx,
            viewport,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn view_entity_hierarchy_ui(
        &self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
        query_result: &DataQueryResult,
        node_or_path: &DataResultNodeOrPath<'_>,
        view: &ViewBlueprint,
        view_visible: bool,
        projection_mode: bool,
    ) {
        let entity_path = node_or_path.path();

        if projection_mode && entity_path == &view.space_origin {
            if ui
                .list_item()
                .show_hierarchical(
                    ui,
                    list_item::LabelContent::new("$origin")
                        .subdued(true)
                        .italics(true)
                        .with_icon(&re_ui::icons::INTERNAL_LINK),
                )
                .on_hover_text(
                    "This subtree corresponds to the View's origin, and is displayed above \
                    the 'Projections' section. Click to select it.",
                )
                .clicked()
            {
                ctx.selection_state().set_selection(Item::DataResult(
                    view.id,
                    InstancePath::entity_all(entity_path.clone()),
                ));
            }
            return;
        }

        let data_result_node = node_or_path.data_result_node();

        let item = Item::DataResult(view.id, entity_path.clone().into());
        let is_selected = ctx.selection().contains_item(&item);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let visible = data_result_node.map_or(false, |n| n.data_result.is_visible(ctx));
        let empty_origin = entity_path == &view.space_origin && data_result_node.is_none();

        let item_label = if entity_path.is_root() {
            "/ (root)".to_owned()
        } else {
            entity_path
                .iter()
                .last()
                .map_or("unknown".to_owned(), |e| e.ui_string())
        };
        let item_label = if ctx.recording().is_known_entity(entity_path) {
            egui::RichText::new(item_label)
        } else {
            ui.ctx().warning_text(item_label)
        };

        let subdued = !view_visible || !visible;

        let mut item_content = list_item::LabelContent::new(item_label)
            .with_icon(guess_instance_path_icon(
                ctx,
                &InstancePath::from(entity_path.clone()),
            ))
            .subdued(subdued);

        let list_item = ui
            .list_item()
            .draggable(true)
            .selected(is_selected)
            .force_hovered(is_item_hovered);

        // We force the origin to be displayed, even if it's fully empty, in which case it can be
        // neither shown/hidden nor removed.
        if !empty_origin {
            item_content = item_content.with_buttons(|ui: &mut egui::Ui| {
                let mut visible_after = visible;
                let vis_response = visibility_button_ui(ui, view_visible, &mut visible_after);
                if visible_after != visible {
                    if let Some(data_result_node) = data_result_node {
                        data_result_node
                            .data_result
                            .save_recursive_override_or_clear_if_redundant(
                                ctx,
                                &query_result.tree,
                                &Visible::from(visible_after),
                            );
                    }
                }

                let response =
                    remove_button_ui(ui, "Remove this entity and all its children from the view");
                if response.clicked() {
                    view.contents
                        .remove_subtree_and_matching_rules(ctx, entity_path.clone());
                }

                response | vis_response
            });
        }

        // If there's any children on the data result nodes, show them, otherwise we're good with this list item as is.
        let has_children = data_result_node.map_or(false, |n| !n.children.is_empty());
        let response = if let (true, Some(node)) = (has_children, data_result_node) {
            // Don't default open projections.
            let default_open = entity_path.starts_with(&view.space_origin)
                && Self::default_open_for_data_result(node);

            // Globally unique id - should only be one of these in view at one time.
            // We do this so that we can support "collapse/expand all" command.
            let id = egui::Id::new(
                CollapseScope::BlueprintTree.data_result(view.id, entity_path.clone()),
            );

            list_item
                .show_hierarchical_with_children(ui, id, default_open, item_content, |ui| {
                    for child in node.children.iter().sorted_by_key(|c| {
                        query_result
                            .tree
                            .lookup_result(**c)
                            .map_or(&view.space_origin, |c| &c.entity_path)
                    }) {
                        let Some(child_node) = query_result.tree.lookup_node(*child) else {
                            debug_assert!(false, "DataResultNode {node:?} has an invalid child");
                            continue;
                        };

                        self.view_entity_hierarchy_ui(
                            ctx,
                            viewport,
                            ui,
                            query_result,
                            &DataResultNodeOrPath::DataResultNode(child_node),
                            view,
                            view_visible,
                            projection_mode,
                        );
                    }
                })
                .item_response
        } else {
            list_item.show_hierarchical(ui, item_content)
        };

        let response = response.on_hover_ui(|ui| {
            let query = ctx.current_query();
            let include_subtree = false;
            re_data_ui::item_ui::entity_hover_card_ui(
                ui,
                ctx,
                &query,
                ctx.recording(),
                entity_path,
                include_subtree,
            );

            if empty_origin {
                ui.label(ui.ctx().warning_text(
                    "This view's query did not match any data under the space origin",
                ));
            }
        });

        context_menu_ui_for_item(
            ctx,
            viewport,
            &item,
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        self.scroll_to_me_if_needed(ui, &item, &response);
        ctx.handle_select_hover_drag_interactions(&response, item, true);
    }

    /// Add a button to trigger the addition of a new view or container.
    #[allow(clippy::unused_self)]
    fn add_new_view_button_ui(
        &self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) {
        if ui
            .small_icon_button(&re_ui::icons::ADD)
            .on_hover_text("Add a new view or container")
            .clicked()
        {
            // If a single container is selected, we use it as target. Otherwise, we target the
            // root container.
            let target_container_id =
                if let Some(Item::Container(container_id)) = ctx.selection().single_item() {
                    *container_id
                } else {
                    viewport.root_container
                };

            show_add_view_or_container_modal(target_container_id);
        }
    }

    // ----------------------------------------------------------------------------
    // drag and drop support

    fn handle_root_container_drag_and_drop_interaction(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &egui::Ui,
        contents: Contents,
        response: &egui::Response,
    ) {
        //
        // check if a drag with acceptable content is in progress
        //

        let Some(dragged_payload) = egui::DragAndDrop::payload::<DragAndDropPayload>(ui.ctx())
        else {
            return;
        };

        let DragAndDropPayload::Contents {
            contents: dragged_contents,
        } = dragged_payload.as_ref()
        else {
            // nothing we care about is being dragged
            return;
        };

        //
        // find the drop target
        //

        // Prepare the item description structure needed by `find_drop_target`. Here, we use
        // `Contents` for the "ItemId" generic type parameter.
        let item_desc = re_ui::drag_and_drop::ItemContext {
            id: contents,
            item_kind: re_ui::drag_and_drop::ItemKind::RootContainer,
            previous_container_id: None,
        };

        let drop_target = re_ui::drag_and_drop::find_drop_target(
            ui,
            &item_desc,
            response.rect,
            None,
            DesignTokens::list_item_height(),
        );

        if let Some(drop_target) = drop_target {
            self.handle_contents_drop_target(ctx, viewport, ui, dragged_contents, &drop_target);
        }
    }

    fn handle_drag_and_drop_interaction(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &egui::Ui,
        contents: Contents,
        response: &egui::Response,
        body_response: Option<&egui::Response>,
    ) {
        //
        // check if a drag with acceptable content is in progress
        //

        let Some(dragged_payload) = egui::DragAndDrop::payload::<DragAndDropPayload>(ui.ctx())
        else {
            return;
        };

        let DragAndDropPayload::Contents {
            contents: dragged_contents,
        } = dragged_payload.as_ref()
        else {
            // nothing we care about is being dragged
            return;
        };

        //
        // find our parent, our position within parent, and the previous container (if any)
        //

        let Some((parent_container_id, position_index_in_parent)) =
            viewport.find_parent_and_position_index(&contents)
        else {
            return;
        };

        let previous_container = if position_index_in_parent > 0 {
            viewport
                .container(&parent_container_id)
                .map(|container| container.contents[position_index_in_parent - 1])
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
            item_kind: match contents {
                Contents::Container(_) => re_ui::drag_and_drop::ItemKind::Container {
                    parent_id: Contents::Container(parent_container_id),
                    position_index_in_parent,
                },
                Contents::View(_) => re_ui::drag_and_drop::ItemKind::Leaf {
                    parent_id: Contents::Container(parent_container_id),
                    position_index_in_parent,
                },
            },
            previous_container_id: previous_container,
        };

        let drop_target = re_ui::drag_and_drop::find_drop_target(
            ui,
            &item_desc,
            response.rect,
            body_response.map(|r| r.rect),
            DesignTokens::list_item_height(),
        );

        if let Some(drop_target) = drop_target {
            self.handle_contents_drop_target(ctx, viewport, ui, dragged_contents, &drop_target);
        }
    }

    fn handle_empty_space_drag_and_drop_interaction(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &egui::Ui,
        empty_space: egui::Rect,
    ) {
        //
        // check if a drag with acceptable content is in progress
        //

        let Some(dragged_payload) = egui::DragAndDrop::payload::<DragAndDropPayload>(ui.ctx())
        else {
            return;
        };

        let DragAndDropPayload::Contents {
            contents: dragged_contents,
        } = dragged_payload.as_ref()
        else {
            // nothing we care about is being dragged
            return;
        };

        //
        // prepare a drop target corresponding to "insert last in root container"
        //
        // TODO(ab): this is a rather primitive behavior. Ideally we should allow dropping in the last container based
        //           on the horizontal position of the cursor.

        if ui.rect_contains_pointer(empty_space) {
            let drop_target = re_ui::drag_and_drop::DropTarget::new(
                empty_space.x_range(),
                empty_space.top(),
                Contents::Container(viewport.root_container),
                usize::MAX,
            );

            self.handle_contents_drop_target(ctx, viewport, ui, dragged_contents, &drop_target);
        }
    }

    fn handle_contents_drop_target(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &Ui,
        dragged_contents: &[Contents],
        drop_target: &DropTarget<Contents>,
    ) {
        // We cannot allow the target location to be "inside" any of the dragged items, because that
        // would amount to moving myself inside of me.
        let parent_contains_dragged_content = |content: &Contents| {
            if let Contents::Container(dragged_container_id) = content {
                if viewport
                    .is_contents_in_container(&drop_target.target_parent_id, dragged_container_id)
                {
                    return true;
                }
            }
            false
        };
        if dragged_contents.iter().any(parent_contains_dragged_content) {
            ctx.drag_and_drop_manager
                .set_feedback(DragAndDropFeedback::Reject);
            return;
        }

        ui.painter().hline(
            drop_target.indicator_span_x,
            drop_target.indicator_position_y,
            (2.0, egui::Color32::WHITE),
        );

        let Contents::Container(target_container_id) = drop_target.target_parent_id else {
            // this shouldn't happen
            ctx.drag_and_drop_manager
                .set_feedback(DragAndDropFeedback::Reject);
            return;
        };

        if ui.input(|i| i.pointer.any_released()) {
            viewport.move_contents(
                dragged_contents.to_vec(),
                target_container_id,
                drop_target.target_position_index,
            );

            egui::DragAndDrop::clear_payload(ui.ctx());
        } else {
            ctx.drag_and_drop_manager
                .set_feedback(DragAndDropFeedback::Accept);
            self.next_candidate_drop_parent_container_id = Some(target_container_id);
        }
    }

    /// Is the provided container the current candidate parent container for the ongoing drag?
    ///
    /// When a drag is in progress, the candidate parent container for the dragged item should be highlighted. Note that
    /// this can happen when hovering said container, its direct children, or even the item just after it.
    fn is_candidate_drop_parent_container(&self, container_id: &ContainerId) -> bool {
        self.candidate_drop_parent_container_id.as_ref() == Some(container_id)
    }
}

// ----------------------------------------------------------------------------

fn reset_blueprint_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    let default_blueprint_id = ctx
        .store_context
        .hub
        .default_blueprint_id_for_app(&ctx.store_context.app_id);

    let default_blueprint = default_blueprint_id.and_then(|id| ctx.store_context.bundle.get(id));

    let mut disabled_reason = None;

    if let Some(default_blueprint) = default_blueprint {
        let active_is_clone_of_default = Some(default_blueprint.store_id()).as_ref()
            == ctx.store_context.blueprint.cloned_from();
        let last_modified_at_the_same_time =
            default_blueprint.latest_row_id() == ctx.store_context.blueprint.latest_row_id();
        if active_is_clone_of_default && last_modified_at_the_same_time {
            disabled_reason = Some("No modifications have been made");
        }
    }

    let enabled = disabled_reason.is_none();
    let response = ui.add_enabled(enabled, ui.small_icon_button_widget(&re_ui::icons::RESET));

    let response = if let Some(disabled_reason) = disabled_reason {
        response.on_disabled_hover_text(disabled_reason)
    } else {
        let hover_text = if default_blueprint_id.is_some() {
            "Reset to the default blueprint for this app"
        } else {
            "Re-populate viewport with automatically chosen views"
        };
        response.on_hover_text(hover_text)
    };

    if response.clicked() {
        ctx.command_sender
            .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);
    }
}

/// Expand all required items and compute which item we should scroll to.
fn handle_focused_item(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &egui::Ui,
    focused_item: &Item,
) -> Option<Item> {
    match focused_item {
        Item::AppId(_) | Item::DataSource(_) | Item::StoreId(_) => None,

        Item::Container(container_id) => {
            expand_all_contents_until(viewport, ui.ctx(), &Contents::Container(*container_id));
            Some(focused_item.clone())
        }
        Item::View(view_id) => {
            expand_all_contents_until(viewport, ui.ctx(), &Contents::View(*view_id));
            ctx.focused_item.clone()
        }
        Item::DataResult(view_id, instance_path) => {
            expand_all_contents_until(viewport, ui.ctx(), &Contents::View(*view_id));
            expand_all_data_results_until(ctx, ui.ctx(), view_id, &instance_path.entity_path);

            ctx.focused_item.clone()
        }
        Item::InstancePath(instance_path) => {
            let view_ids = list_views_with_entity(ctx, viewport, &instance_path.entity_path);

            // focus on the first matching data result
            let res = view_ids
                .first()
                .map(|id| Item::DataResult(*id, instance_path.clone()));

            for view_id in view_ids {
                expand_all_contents_until(viewport, ui.ctx(), &Contents::View(view_id));
                expand_all_data_results_until(ctx, ui.ctx(), &view_id, &instance_path.entity_path);
            }

            res
        }
        Item::ComponentPath(component_path) => handle_focused_item(
            ctx,
            viewport,
            ui,
            &Item::InstancePath(InstancePath::entity_all(component_path.entity_path.clone())),
        ),
    }
}

/// Expand all containers until reaching the provided content.
fn expand_all_contents_until(
    viewport: &ViewportBlueprint,
    egui_ctx: &egui::Context,
    focused_contents: &Contents,
) {
    //TODO(ab): this could look nicer if `Contents` was declared in re_view_context :)
    let expend_contents = |contents: &Contents| match contents {
        Contents::Container(container_id) => CollapseScope::BlueprintTree
            .container(*container_id)
            .set_open(egui_ctx, true),
        Contents::View(view_id) => CollapseScope::BlueprintTree
            .view(*view_id)
            .set_open(egui_ctx, true),
    };

    viewport.visit_contents(&mut |contents, hierarchy| {
        if contents == focused_contents {
            expend_contents(contents);
            for parent in hierarchy {
                expend_contents(&Contents::Container(*parent));
            }
        }
    });
}

/// List all views that have the provided entity as data result.
#[inline]
fn list_views_with_entity(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    entity_path: &EntityPath,
) -> SmallVec<[ViewId; 4]> {
    let mut view_ids = SmallVec::new();
    viewport.visit_contents(&mut |contents, _| {
        if let Contents::View(view_id) = contents {
            let result_tree = &ctx.lookup_query_result(*view_id).tree;
            if result_tree.lookup_node_by_path(entity_path).is_some() {
                view_ids.push(*view_id);
            }
        }
    });
    view_ids
}

/// Expand data results of the provided view all the way to the provided entity.
fn expand_all_data_results_until(
    ctx: &ViewerContext<'_>,
    egui_ctx: &egui::Context,
    view_id: &ViewId,
    entity_path: &EntityPath,
) {
    let result_tree = &ctx.lookup_query_result(*view_id).tree;
    if result_tree.lookup_node_by_path(entity_path).is_some() {
        if let Some(root_node) = result_tree.root_node() {
            EntityPath::incremental_walk(Some(&root_node.data_result.entity_path), entity_path)
                .chain(std::iter::once(root_node.data_result.entity_path.clone()))
                .for_each(|entity_path| {
                    CollapseScope::BlueprintTree
                        .data_result(*view_id, entity_path)
                        .set_open(egui_ctx, true);
                });
        }
    }
}

fn remove_button_ui(ui: &mut Ui, tooltip: &str) -> Response {
    ui.small_icon_button(&re_ui::icons::REMOVE)
        .on_hover_text(tooltip)
}

fn visibility_button_ui(ui: &mut egui::Ui, enabled: bool, visible: &mut bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        ui.visibility_toggle_button(visible)
            .on_hover_text("Toggle visibility")
            .on_disabled_hover_text("A parent is invisible")
    })
    .inner
}
