use egui::{Response, Ui, WidgetInfo, WidgetType};
use re_context_menu::{SelectionUpdateBehavior, context_menu_ui_for_item_with_context};
use re_data_ui::item_ui::guess_instance_path_icon;
use re_entity_db::InstancePath;
use re_log_types::{ApplicationId, EntityPath, EntityPathHash};
use re_ui::drag_and_drop::DropTarget;
use re_ui::filter_widget::format_matching_text;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{ContextExt as _, DesignTokens, UiExt as _, filter_widget, list_item};
use re_viewer_context::{
    CollapseScope, ContainerId, Contents, DragAndDropFeedback, DragAndDropPayload, HoverHighlight,
    Item, ItemCollection, ItemContext, PerVisualizerType, SystemCommand, SystemCommandSender as _,
    ViewId, ViewStates, ViewerContext, VisitorControlFlow, VisualizerReportSeverity,
    VisualizerTypeReport, contents_name_style, icon_for_container_kind,
};
use re_viewport_blueprint::ViewportBlueprint;
use re_viewport_blueprint::ui::show_add_view_or_container_modal;
use smallvec::SmallVec;

use crate::data::{
    BlueprintTreeData, ContainerData, ContentsData, DataResultData, DataResultKind, ViewData,
};

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

    /// Candidate parent container to be drawn on the next frame.
    ///
    /// We double-buffer this value to deal with ordering constraints.
    next_candidate_drop_parent_container_id: Option<ContainerId>,

    /// State of the entity filter widget.
    filter_state: filter_widget::FilterState,

    /// The store id the filter widget relates to.
    ///
    /// Used to invalidate the filter state (aka deactivate it) when the user switches to a
    /// recording with a different application id.
    filter_state_app_id: Option<ApplicationId>,

    /// Range selection anchor item.
    ///
    /// This is the item we used as a starting point for range selection. It is set and remembered
    /// everytime the user clicks on an item _without_ holding shift.
    range_selection_anchor_item: Option<Item>,

    /// Used when the selection is modified using key navigation.
    ///
    /// IMPORTANT: Always make sure that the item will be drawn this or next frame when setting this
    /// to `Some`, so that this flag is immediately consumed.
    scroll_to_me_item: Option<Item>,
}

impl BlueprintTree {
    /// Activates the search filter (for e.g. test purposes).
    pub fn activate_filter(&mut self, query: &str) {
        self.filter_state.activate(query);
    }

    /// Show the Blueprint section of the left panel based on the current [`ViewportBlueprint`]
    pub fn show(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
        view_states: &ViewStates,
    ) {
        re_tracing::profile_function!();

        // Invalidate the filter widget if the store id has changed.
        if self.filter_state_app_id.as_ref() != Some(ctx.store_context.application_id()) {
            self.filter_state = Default::default();
            self.filter_state_app_id = Some(ctx.store_context.application_id().clone());
        }

        ui.panel_content(|ui| {
            ui.list_item_scope("blueprint_section_title", |ui| {
                ui.list_item().interactive(false).show_flat(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        let title_response = self
                            .filter_state
                            .section_title_ui(ui, egui::RichText::new("Blueprint").strong());

                        if let Some(title_response) = title_response {
                            title_response.on_hover_text(
                                "The blueprint is where you can configure the Rerun Viewer",
                            );
                        }
                    })
                    .menu_button(
                        &re_ui::icons::MORE,
                        "Open menu with more options",
                        |ui| {
                            add_new_view_or_container_menu_button(ctx, viewport_blueprint, ui);
                            set_blueprint_to_default_menu_buttons(ctx, ui);
                            set_blueprint_to_auto_menu_button(ctx, ui);
                        },
                    ),
                );
            });
        });

        // This call is excluded from `panel_content` because it has a ScrollArea, which should not be
        // inset. Instead, it calls panel_content itself inside the ScrollArea.
        self.tree_ui(ctx, viewport_blueprint, ui, view_states);
    }

    /// Show the blueprint panel tree view.
    fn tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
        view_states: &ViewStates,
    ) {
        re_tracing::profile_function!();

        // The candidate drop parent container is double-buffered, so here we have the buffer swap.
        self.candidate_drop_parent_container_id = self.next_candidate_drop_parent_container_id;
        self.next_candidate_drop_parent_container_id = None;

        let blueprint_tree_data = BlueprintTreeData::from_blueprint_and_filter(
            ctx,
            viewport_blueprint,
            &self.filter_state.filter(),
        );

        egui::ScrollArea::both()
            .id_salt("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                re_tracing::profile_scope!("blueprint_tree_scroll_area");
                ui.panel_content(|ui| {
                    self.blueprint_tree_scroll_to_item =
                        ctx.focused_item.as_ref().and_then(|item| {
                            self.handle_focused_item(ctx, viewport_blueprint, ui, item)
                        });

                    list_item::list_item_scope(ui, "blueprint tree", |ui| {
                        if let Some(root_container) = &blueprint_tree_data.root_container {
                            self.root_container_ui(
                                ctx,
                                viewport_blueprint,
                                &blueprint_tree_data,
                                ui,
                                root_container,
                                view_states,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        WidgetInfo::labeled(WidgetType::Panel, true, "_blueprint_tree")
                    });

                    let empty_space_response =
                        ui.allocate_response(ui.available_size(), egui::Sense::click());

                    // clear selection upon clicking on empty space
                    if empty_space_response.clicked() {
                        ctx.command_sender()
                            .send_system(SystemCommand::clear_selection());
                    }

                    // handle drag and drop interaction on empty space
                    self.handle_empty_space_drag_and_drop_interaction(
                        ctx,
                        viewport_blueprint,
                        ui,
                        empty_space_response.rect,
                    );
                });
            });
    }

    /// Display the root container.
    ///
    /// The root container is different from other containers in that it cannot be removed or dragged, and it cannot be
    /// collapsed, so it's drawn without a collapsing triangle.
    fn root_container_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &mut egui::Ui,
        container_data: &ContainerData,
        view_states: &ViewStates,
    ) {
        re_tracing::profile_function!();

        let item = Item::Container(container_data.id);

        // It's possible that the root becomes technically collapsed (e.g. context menu or arrow
        // navigation), even though we don't allow that in the ui. We really don't want that,
        // though, because it breaks the collapse-based tree data visiting. To avoid that, we always
        // force uncollapse this item.
        self.collapse_scope()
            .container(container_data.id)
            .set_open(ctx.egui_ctx(), true);

        let item_response = ui
            .list_item()
            .render_offscreen(false)
            .selected(ctx.selection().contains_item(&item))
            .draggable(true) // allowed for consistency but results in an invalid drag
            .drop_target_style(self.is_candidate_drop_parent_container(&container_data.id))
            .show_flat(
                ui,
                list_item::LabelContent::new(format!(
                    "Viewport ({})",
                    container_data.name.as_ref()
                ))
                .label_style(contents_name_style(&container_data.name))
                .with_icon(icon_for_container_kind(&container_data.kind))
                .subdued(!container_data.visible)
                .with_buttons(|ui| {
                    // If this has been hidden in a blueprint we want to be
                    // able to make it visible again in the viewer.
                    if !container_data.visible {
                        let mut visible_after = container_data.visible;
                        visibility_button_ui(ui, true, &mut visible_after);
                        if visible_after != container_data.visible {
                            viewport_blueprint.set_content_visibility(
                                ctx,
                                &Contents::Container(viewport_blueprint.root_container),
                                visible_after,
                            );
                        }
                    }
                }),
            );

        for child in &container_data.children {
            self.contents_ui(
                ctx,
                viewport_blueprint,
                blueprint_tree_data,
                ui,
                child,
                container_data.visible,
                view_states,
            );
        }

        self.handle_interactions_for_item(
            ctx,
            viewport_blueprint,
            blueprint_tree_data,
            ui,
            &item,
            &item_response,
        );

        self.handle_root_container_drag_and_drop_interaction(
            ctx,
            viewport_blueprint,
            ui,
            Contents::Container(container_data.id),
            &item_response,
        );
    }

    #[expect(clippy::too_many_arguments)]
    fn contents_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &mut egui::Ui,
        contents_data: &ContentsData,
        parent_visible: bool,
        view_states: &ViewStates,
    ) {
        match contents_data {
            ContentsData::Container(container_data) => {
                self.container_ui(
                    ctx,
                    viewport_blueprint,
                    blueprint_tree_data,
                    ui,
                    container_data,
                    parent_visible,
                    view_states,
                );
            }
            ContentsData::View(view_data) => {
                self.view_ui(
                    ctx,
                    viewport_blueprint,
                    blueprint_tree_data,
                    ui,
                    view_data,
                    parent_visible,
                    view_states.per_visualizer_type_reports(view_data.id),
                );
            }
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn container_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &mut egui::Ui,
        container_data: &ContainerData,
        parent_visible: bool,
        view_states: &ViewStates,
    ) {
        re_tracing::profile_function!();

        let item = Item::Container(container_data.id);
        let content = Contents::Container(container_data.id);

        let mut visible = container_data.visible;
        let container_visible = visible && parent_visible;

        let item_content = list_item::LabelContent::new(container_data.name.as_ref())
            .subdued(!container_visible)
            .label_style(contents_name_style(&container_data.name))
            .with_icon(icon_for_container_kind(&container_data.kind))
            .with_buttons(|ui| {
                visibility_button_ui(ui, parent_visible, &mut visible);

                if remove_button_ui(ui, "Remove container").clicked() {
                    viewport_blueprint.mark_user_interaction(ctx);
                    viewport_blueprint.remove_contents(content);
                }
            });

        // Globally unique id - should only be one of these in view at one time.
        // We do this so that we can support "collapse/expand all" command.
        let id = egui::Id::new(self.collapse_scope().container(container_data.id));

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .render_offscreen(false)
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .drop_target_style(self.is_candidate_drop_parent_container(&container_data.id))
            .show_hierarchical_with_children(
                ui,
                id,
                container_data.default_open,
                item_content,
                |ui| {
                    for child in &container_data.children {
                        self.contents_ui(
                            ctx,
                            viewport_blueprint,
                            blueprint_tree_data,
                            ui,
                            child,
                            container_visible,
                            view_states,
                        );
                    }
                },
            );

        viewport_blueprint.set_content_visibility(ctx, &content, visible);
        let response = response.on_hover_text(format!("{:?} container", container_data.kind));

        self.handle_interactions_for_item(
            ctx,
            viewport_blueprint,
            blueprint_tree_data,
            ui,
            &item,
            &response,
        );

        self.handle_drag_and_drop_interaction(
            ctx,
            viewport_blueprint,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    #[expect(clippy::too_many_arguments)]
    fn view_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &mut egui::Ui,
        view_data: &ViewData,
        container_visible: bool,
        errors: Option<&PerVisualizerType<VisualizerTypeReport>>,
    ) {
        re_tracing::profile_function!();

        let mut visible = view_data.visible;
        let view_visible = visible && container_visible;
        let item = Item::View(view_data.id);

        let class = ctx
            .view_class_registry()
            .get_class_or_log_error(view_data.class_identifier);

        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let item_content = if errors.is_some_and(|errors| !errors.is_empty()) {
            list_item::LabelContent::new(
                egui::RichText::new(view_data.name.as_ref()).color(ui.visuals().error_fg_color),
            )
        } else {
            list_item::LabelContent::new(view_data.name.as_ref())
        };

        let item_content = item_content
            .label_style(contents_name_style(&view_data.name))
            .with_icon(class.icon())
            .subdued(!view_visible)
            .with_buttons(|ui| {
                visibility_button_ui(ui, container_visible, &mut visible);

                if remove_button_ui(ui, "Remove view from the viewport").clicked() {
                    viewport_blueprint.mark_user_interaction(ctx);
                    viewport_blueprint.remove_contents(Contents::View(view_data.id));
                }
            });

        // Globally unique id - should only be one of these in view at one time.
        // We do this so that we can support "collapse/expand all" command.
        let id = egui::Id::new(self.collapse_scope().view(view_data.id));

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .render_offscreen(false)
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .force_hovered(is_item_hovered)
            .show_hierarchical_with_children(ui, id, view_data.default_open, item_content, |ui| {
                if let Some(data_result_data) = &view_data.origin_tree {
                    self.data_result_ui(
                        ctx,
                        viewport_blueprint,
                        blueprint_tree_data,
                        ui,
                        data_result_data,
                        view_visible,
                        errors,
                    );
                }

                if !view_data.projection_trees.is_empty() {
                    ui.list_item()
                        .render_offscreen(false)
                        .interactive(false)
                        .show_flat(
                            ui,
                            list_item::LabelContent::new("Projections:").italics(true),
                        );

                    for projection in &view_data.projection_trees {
                        self.data_result_ui(
                            ctx,
                            viewport_blueprint,
                            blueprint_tree_data,
                            ui,
                            projection,
                            view_visible,
                            errors,
                        );
                    }
                }
            });

        let response = response.on_hover_text(format!("{} view", class.display_name()));

        if response.clicked() {
            viewport_blueprint.focus_tab(view_data.id);
        }

        let content = Contents::View(view_data.id);
        viewport_blueprint.set_content_visibility(ctx, &content, visible);

        self.handle_interactions_for_item(
            ctx,
            viewport_blueprint,
            blueprint_tree_data,
            ui,
            &item,
            &response,
        );

        self.handle_drag_and_drop_interaction(
            ctx,
            viewport_blueprint,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    #[expect(clippy::too_many_arguments)]
    fn data_result_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &mut egui::Ui,
        data_result_data: &DataResultData,
        view_visible: bool,
        visualizer_reports: Option<&PerVisualizerType<VisualizerTypeReport>>,
    ) {
        let item = Item::DataResult(
            data_result_data.view_id,
            data_result_data.entity_path.clone().into(),
        );

        let item_content = match data_result_data.kind {
            DataResultKind::EmptyOriginPlaceholder | DataResultKind::EntityPart => {
                let is_empty_origin_placeholder = matches!(
                    data_result_data.kind,
                    DataResultKind::EmptyOriginPlaceholder
                );

                let highest_report_severity: Option<VisualizerReportSeverity> = visualizer_reports
                    .and_then(|visualizer_reports| {
                        data_result_data
                            .visualizer_instruction_ids
                            .iter()
                            .filter_map(|instruction_id| {
                                visualizer_reports
                                    .values()
                                    .filter_map(|err| err.highest_severity_for(instruction_id))
                                    .max()
                            })
                            .max()
                    });

                let format_color = match highest_report_severity {
                    Some(
                        VisualizerReportSeverity::Error
                        | VisualizerReportSeverity::OverallVisualizerError,
                    ) => Some(ui.visuals().error_fg_color),
                    Some(VisualizerReportSeverity::Warning) => Some(ui.visuals().warn_fg_color),
                    None => is_empty_origin_placeholder.then(|| ui.visuals().warn_fg_color),
                };

                let item_content = list_item::LabelContent::new(format_matching_text(
                    ctx.egui_ctx(),
                    &data_result_data.label,
                    data_result_data.highlight_sections.iter().cloned(),
                    format_color,
                ))
                .with_icon(guess_instance_path_icon(
                    ctx,
                    &data_result_data.instance_path(),
                ));

                if is_empty_origin_placeholder {
                    item_content.subdued(true)
                } else {
                    item_content
                        .subdued(!view_visible || !data_result_data.visible)
                        .with_buttons(|ui: &mut egui::Ui| {
                            let mut visible_after = data_result_data.visible;
                            visibility_button_ui(ui, view_visible, &mut visible_after);
                            if visible_after != data_result_data.visible {
                                data_result_data.update_visibility(ctx, visible_after);
                            }

                            if remove_button_ui(
                                ui,
                                "Remove this entity and all its children from the view",
                            )
                            .clicked()
                            {
                                data_result_data
                                    .remove_data_result_from_view(ctx, viewport_blueprint);
                            }
                        })
                }
            }

            DataResultKind::OriginProjectionPlaceholder => {
                if ui
                    .list_item()
                    .render_offscreen(false)
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new("$origin")
                            .subdued(true)
                            .italics(true)
                            .with_icon(&re_ui::icons::INTERNAL_LINK),
                    )
                    .on_hover_text(
                        "This subtree corresponds to the view's origin, and is displayed above \
                        the 'Projections' section. Click to select it.",
                    )
                    .clicked()
                {
                    ctx.command_sender()
                        .send_system(SystemCommand::set_selection(item));
                }

                return;
            }
        };

        let is_selected = ctx.selection().contains_item(&item);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let list_item = ui
            .list_item()
            .render_offscreen(false)
            .draggable(true)
            .selected(is_selected)
            .force_hovered(is_item_hovered);

        // If there's any children on the data result nodes, show them, otherwise we're good with this list item as is.
        let has_children = !data_result_data.children.is_empty(); //data_result_node.is_some_and(|n| !n.children.is_empty());
        let response = if has_children {
            // Globally unique id - should only be one of these in view at one time.
            // We do this so that we can support "collapse/expand all" command.
            let id = egui::Id::new(self.collapse_scope().data_result(
                data_result_data.view_id,
                data_result_data.entity_path.clone(),
            ));

            list_item
                .show_hierarchical_with_children(
                    ui,
                    id,
                    data_result_data.default_open,
                    item_content,
                    |ui| {
                        for child in &data_result_data.children {
                            self.data_result_ui(
                                ctx,
                                viewport_blueprint,
                                blueprint_tree_data,
                                ui,
                                child,
                                view_visible,
                                visualizer_reports,
                            );
                        }
                    },
                )
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
                &data_result_data.entity_path,
                include_subtree,
            );

            if matches!(
                data_result_data.kind,
                DataResultKind::EmptyOriginPlaceholder
            ) {
                ui.label(ui.ctx().warning_text(
                    "This view's query did not match any data under the space origin",
                ));
            }
        });

        self.handle_interactions_for_item(
            ctx,
            viewport_blueprint,
            blueprint_tree_data,
            ui,
            &item,
            &response,
        );
    }

    // ----------------------------------------------------------------------------
    // item interactions

    fn handle_interactions_for_item(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        blueprint_tree_data: &BlueprintTreeData,
        ui: &egui::Ui,
        item: &Item,
        response: &Response,
    ) {
        context_menu_ui_for_item_with_context(
            ctx,
            viewport_blueprint,
            item,
            // expand/collapse context menu actions need this information
            ItemContext::BlueprintTree {
                filter_session_id: self.filter_state.session_id(),
            },
            response,
            SelectionUpdateBehavior::UseSelection,
        );
        self.scroll_to_me_if_needed(ui, item, response);
        ctx.handle_select_hover_drag_interactions(response, item.clone(), true);
        ctx.handle_select_focus_sync(response, item.clone());

        self.handle_range_selection(ctx, blueprint_tree_data, item.clone(), response);
    }

    /// Handle setting/extending the selection based on shift-clicking.
    fn handle_range_selection(
        &mut self,
        ctx: &ViewerContext<'_>,
        blueprint_tree_data: &BlueprintTreeData,
        item: Item,
        response: &Response,
    ) {
        // Early out if we're not being clicked.
        if !response.clicked() {
            return;
        }

        let modifiers = ctx.egui_ctx().input(|i| i.modifiers);

        if modifiers.shift {
            if let Some(anchor_item) = &self.range_selection_anchor_item {
                let items_in_range = Self::items_in_range(
                    ctx,
                    blueprint_tree_data,
                    self.collapse_scope(),
                    anchor_item,
                    &item,
                );

                if items_in_range.is_empty() {
                    // This can happen if the last clicked item became invisible due to collapsing, or if
                    // the user switched to another recording. In either case, we invalidate it.
                    self.range_selection_anchor_item = None;
                } else {
                    let items = ItemCollection::from_items_and_context(
                        items_in_range.into_iter().map(|item| {
                            (
                                item,
                                Some(ItemContext::BlueprintTree {
                                    filter_session_id: self.filter_state.session_id(),
                                }),
                            )
                        }),
                    );

                    if modifiers.command {
                        // We extend into the current selection to append new items at the end.
                        let mut selection = ctx.selection().clone();
                        selection.extend(items);
                        ctx.command_sender()
                            .send_system(SystemCommand::set_selection(selection));
                    } else {
                        ctx.command_sender()
                            .send_system(SystemCommand::set_selection(items));
                    }
                }
            }
        } else {
            self.range_selection_anchor_item = Some(item);
        }
    }

    /// Selects a range of items in the blueprint tree.
    ///
    /// This method selects all [`Item`]s displayed between the provided shift-clicked item and the
    /// existing last-clicked item (if any). It takes into account the collapsed state, so only
    /// actually visible items may be selected.
    fn items_in_range(
        ctx: &ViewerContext<'_>,
        blueprint_tree_data: &BlueprintTreeData,
        collapse_scope: CollapseScope,
        anchor_item: &Item,
        shift_clicked_item: &Item,
    ) -> Vec<Item> {
        let mut items_in_range = vec![];
        let mut found_anchor_item = false;
        let mut found_shift_clicked_items = false;

        let _ignored = blueprint_tree_data.visit(|blueprint_tree_item| {
            let item = blueprint_tree_item.item();

            if &item == anchor_item {
                found_anchor_item = true;
            }

            if &item == shift_clicked_item {
                found_shift_clicked_items = true;
            }

            if found_anchor_item || found_shift_clicked_items {
                items_in_range.push(item);
            }

            if found_anchor_item && found_shift_clicked_items {
                return VisitorControlFlow::Break(());
            }

            let is_expanded = blueprint_tree_item.is_open(ctx.egui_ctx(), collapse_scope);

            if is_expanded {
                VisitorControlFlow::Continue
            } else {
                VisitorControlFlow::SkipBranch
            }
        });

        if !found_anchor_item || !found_shift_clicked_items {
            vec![]
        } else {
            items_in_range
        }
    }

    /// Check if the provided item should be scrolled to.
    fn scroll_to_me_if_needed(&mut self, ui: &egui::Ui, item: &Item, response: &egui::Response) {
        if Some(item) == self.blueprint_tree_scroll_to_item.as_ref() {
            // Scroll only if the entity isn't already visible. This is important because that's what
            // happens when double-clicking an entity _in the blueprint tree_. In such case, it would be
            // annoying to induce a scroll motion.
            if !ui.clip_rect().contains_rect(response.rect) {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }

        if Some(item) == self.scroll_to_me_item.as_ref() {
            // This is triggered by keyboard navigation, so in this case we just want to scroll
            // minimally for the item to be visible.
            response.scroll_to_me(None);
            self.scroll_to_me_item = None;
        }
    }

    // ----------------------------------------------------------------------------
    // view/container drag and drop support

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
            if let Contents::Container(dragged_container_id) = content
                && viewport
                    .is_contents_in_container(&drop_target.target_parent_id, dragged_container_id)
            {
                return true;
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
            (2.0, ui.tokens().strong_fg_color),
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

    pub fn collapse_scope(&self) -> CollapseScope {
        match self.filter_state.session_id() {
            None => CollapseScope::BlueprintTree,
            Some(session_id) => CollapseScope::BlueprintTreeFiltered { session_id },
        }
    }

    // ---

    /// Expand all required items and compute which item we should scroll to.
    fn handle_focused_item(
        &self,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        ui: &egui::Ui,
        focused_item: &Item,
    ) -> Option<Item> {
        match focused_item {
            Item::AppId(_)
            | Item::TableId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_) => None,

            Item::Container(container_id) => {
                self.expand_all_contents_until(
                    viewport,
                    ui.ctx(),
                    &Contents::Container(*container_id),
                );
                Some(focused_item.clone())
            }
            Item::View(view_id) => {
                self.expand_all_contents_until(viewport, ui.ctx(), &Contents::View(*view_id));
                ctx.focused_item.clone()
            }
            Item::DataResult(view_id, instance_path) => {
                self.expand_all_contents_until(viewport, ui.ctx(), &Contents::View(*view_id));
                self.expand_all_data_results_until(
                    ctx,
                    ui.ctx(),
                    view_id,
                    &instance_path.entity_path,
                );

                ctx.focused_item.clone()
            }
            Item::InstancePath(instance_path) => {
                let view_ids =
                    list_views_with_entity(ctx, viewport, instance_path.entity_path.hash());

                // focus on the first matching data result
                let res = view_ids
                    .first()
                    .map(|id| Item::DataResult(*id, instance_path.clone()));

                for view_id in view_ids {
                    self.expand_all_contents_until(viewport, ui.ctx(), &Contents::View(view_id));
                    self.expand_all_data_results_until(
                        ctx,
                        ui.ctx(),
                        &view_id,
                        &instance_path.entity_path,
                    );
                }

                res
            }
            Item::ComponentPath(component_path) => self.handle_focused_item(
                ctx,
                viewport,
                ui,
                &Item::InstancePath(InstancePath::entity_all(component_path.entity_path.clone())),
            ),
        }
    }

    /// Expand all containers until reaching the provided content.
    fn expand_all_contents_until(
        &self,
        viewport: &ViewportBlueprint,
        egui_ctx: &egui::Context,
        focused_contents: &Contents,
    ) {
        let _ignored = viewport.visit_contents(&mut |contents, hierarchy| {
            if contents == focused_contents {
                self.collapse_scope()
                    .contents(*contents)
                    .set_open(egui_ctx, true);

                for parent in hierarchy {
                    self.collapse_scope()
                        .container(*parent)
                        .set_open(egui_ctx, true);
                }

                VisitorControlFlow::Break(())
            } else {
                VisitorControlFlow::Continue
            }
        });
    }

    /// Expand data results of the provided view all the way to the provided entity.
    fn expand_all_data_results_until(
        &self,
        ctx: &ViewerContext<'_>,
        egui_ctx: &egui::Context,
        view_id: &ViewId,
        entity_path: &EntityPath,
    ) {
        let result_tree = &ctx.lookup_query_result(*view_id).tree;
        if result_tree
            .lookup_node_by_path(entity_path.hash())
            .is_some()
            && let Some(root_node) = result_tree.root_node()
        {
            EntityPath::incremental_walk(Some(&root_node.data_result.entity_path), entity_path)
                .chain(std::iter::once(root_node.data_result.entity_path.clone()))
                .for_each(|entity_path| {
                    self.collapse_scope()
                        .data_result(*view_id, entity_path)
                        .set_open(egui_ctx, true);
                });
        }
    }
}

// ----------------------------------------------------------------------------

/// Add a button to trigger the addition of a new view or container.
fn add_new_view_or_container_menu_button(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
) {
    if ui
        .add(re_ui::icons::ADD.as_button_with_label(ui.tokens(), "Add view or container…"))
        .clicked()
    {
        ui.close();

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

fn set_blueprint_to_default_menu_buttons(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    let default_blueprint_id = ctx
        .storage_context
        .hub
        .default_blueprint_id_for_app(ctx.store_context.application_id());

    let default_blueprint = default_blueprint_id.and_then(|id| ctx.storage_context.bundle.get(id));

    let disabled_reason = match default_blueprint {
        None => Some("No default blueprint is set for this app"),
        Some(default_blueprint) => {
            let active_is_clone_of_default =
                Some(default_blueprint.store_id()) == ctx.store_context.blueprint.cloned_from();
            let last_modified_at_the_same_time =
                default_blueprint.latest_row_id() == ctx.store_context.blueprint.latest_row_id();
            if active_is_clone_of_default && last_modified_at_the_same_time {
                Some("No modifications have been made")
            } else {
                None // it is valid to reset to default
            }
        }
    };

    let enabled = disabled_reason.is_none();
    let mut response = ui
        .add_enabled(
            enabled,
            re_ui::icons::RESET.as_button_with_label(ui.tokens(), "Reset to default blueprint"),
        )
        .on_hover_text("Reset to the default blueprint for this app");

    if let Some(disabled_reason) = disabled_reason {
        response = response.on_disabled_hover_text(disabled_reason);
    }

    if response.clicked() {
        ui.close();
        ctx.command_sender()
            .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);
    }
}

fn set_blueprint_to_auto_menu_button(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    // Figuring out when resetting to heuristic blueprint is not changing anything is actually quite hard.
    // There's a wide variety of things to consider that aren't easily caught:
    // * does running view-generation/layout-generation change anything?
    //    * these heuristics run incrementally, does rerunning them in bulk change anything?
    // * any changes in overrides/defaults/view-property means that a reset would change something
    let enabled = true;

    if ui
        .add_enabled(
            enabled,
            re_ui::icons::RESET.as_button_with_label(ui.tokens(), "Reset to heuristic blueprint"),
        )
        .on_hover_text("Re-populate viewport with automatically chosen views")
        .clicked()
    {
        ui.close();
        ctx.command_sender()
            .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprintAndEnableHeuristics);
    }
}

/// List all views that have the provided entity as data result.
#[inline]
fn list_views_with_entity(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    entity_path: EntityPathHash,
) -> SmallVec<[ViewId; 4]> {
    let mut view_ids = SmallVec::new();
    let _ignored = viewport.visit_contents::<()>(&mut |contents, _| {
        if let Contents::View(view_id) = contents {
            let result_tree = &ctx.lookup_query_result(*view_id).tree;
            if result_tree.lookup_node_by_path(entity_path).is_some() {
                view_ids.push(*view_id);
            }
        }

        VisitorControlFlow::Continue
    });
    view_ids
}

fn remove_button_ui(ui: &mut Ui, alt_text_and_tooltip: &str) -> Response {
    ui.small_icon_button(&re_ui::icons::REMOVE, alt_text_and_tooltip)
        .on_hover_text(alt_text_and_tooltip)
}

fn visibility_button_ui(ui: &mut egui::Ui, enabled: bool, visible: &mut bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        ui.visibility_toggle_button(visible)
            .on_hover_text("Toggle visibility")
            .on_disabled_hover_text("A parent is invisible")
    })
    .inner
}
