//! The viewport panel.
//!
//! Contains all views.

use ahash::HashMap;
use egui::remap_clamp;
use egui_tiles::{Behavior as _, EditAction};

use re_context_menu::{SelectionUpdateBehavior, context_menu_ui_for_item};
use re_log_types::{EntityPath, ResolvedEntityPathRule, RuleEffect};
use re_ui::{ContextExt as _, Help, Icon, IconText, UiExt as _, design_tokens_of_visuals};
use re_view::controls::TOGGLE_MAXIMIZE_VIEW;
use re_viewer_context::{
    Contents, DragAndDropFeedback, DragAndDropPayload, Item, PublishedViewInfo,
    SystemExecutionOutput, ViewId, ViewQuery, ViewStates, ViewerContext, icon_for_container_kind,
};
use re_viewport_blueprint::{
    ViewBlueprint, ViewportBlueprint, ViewportCommand, create_entity_add_info,
};

use crate::system_execution::{execute_systems_for_all_views, execute_systems_for_view};

// ----------------------------------------------------------------------------

/// Defines the UI and layout of the Viewport.
pub struct ViewportUi {
    /// The blueprint that drives this viewport.
    /// This is the source of truth from the store for this frame.
    /// All modifications are accumulated in [`ViewportBlueprint::deferred_commands`] and applied at the end of the frame.
    pub blueprint: ViewportBlueprint,
}

impl ViewportUi {
    pub fn new(blueprint: ViewportBlueprint) -> Self {
        Self { blueprint }
    }

    pub fn viewport_ui(
        &self,
        ui: &mut egui::Ui,
        ctx: &ViewerContext<'_>,
        view_states: &mut ViewStates,
    ) {
        let tokens = ui.tokens();

        let Self { blueprint } = self;

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport || !ui.is_visible() {
            return;
        }

        let mut maximized = blueprint.maximized;

        if let Some(view_id) = blueprint.maximized {
            if !blueprint.views.contains_key(&view_id) {
                maximized = None;
            } else if let Some(tile_id) = blueprint.tree.tiles.find_pane(&view_id) {
                if !blueprint.tree.tiles.is_visible(tile_id) {
                    maximized = None;
                }
            }
        }

        let (animating_view_id, animated_rect) = ui
            .data(|data| data.get_temp::<MaximizeAnimationState>(egui::Id::NULL))
            .unwrap_or_default()
            .animated_view_and_rect(ui.ctx(), ui.max_rect());

        let mut tree = if let Some(view_id) = blueprint.maximized.or(animating_view_id) {
            let mut tiles = egui_tiles::Tiles::default();

            // we must ensure that our temporary tree has the correct tile id, such that the tile id
            // to view id logic later in this function works correctly
            let tile_id = Contents::View(view_id).as_tile_id();
            tiles.insert(tile_id, egui_tiles::Tile::Pane(view_id));
            egui_tiles::Tree::new("viewport_tree", tile_id, tiles)
        } else {
            blueprint.tree.clone()
        };

        let executed_systems_per_view =
            execute_systems_for_all_views(ctx, &tree, &blueprint.views, view_states);

        let contents_per_tile_id = blueprint
            .contents_iter()
            .map(|contents| (contents.as_tile_id(), contents))
            .collect();

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = tokens.view_padding() as f32;

            re_tracing::profile_scope!("tree.ui");

            *ui = ui.new_child(egui::UiBuilder::new().max_rect(animated_rect));

            let mut egui_tiles_delegate = TilesDelegate {
                view_states,
                ctx,
                viewport_blueprint: blueprint,
                maximized: &mut maximized,
                executed_systems_per_view,
                contents_per_tile_id,
                edited: false,
                tile_dropped: false,
            };

            tree.ui(&mut egui_tiles_delegate, ui);

            let dragged_payload = egui::DragAndDrop::payload::<DragAndDropPayload>(ui.ctx());
            let dragged_payload = dragged_payload.as_ref().and_then(|payload| {
                if let DragAndDropPayload::Entities { entities } = payload.as_ref() {
                    Some(entities)
                } else {
                    None
                }
            });

            // Outline hovered & selected tiles:
            for contents in blueprint.contents_iter() {
                let tile_id = contents.as_tile_id();
                if let Some(rect) = tree.tiles.rect(tile_id) {
                    let item = contents.as_item();

                    let mut hovered = ctx.hovered().contains_item(&item);
                    let selected = ctx.selection().contains_item(&item);

                    if hovered && ui.rect_contains_pointer(rect) {
                        // Showing a hover-outline when hovering the same thing somewhere else
                        // (e.g. in the blueprint panel) is really helpful,
                        // but showing a hover-outline when just dragging around the camera is
                        // just annoying.
                        hovered = false;
                    }

                    // Handle drag-and-drop if this is a view.
                    //TODO(#8428): simplify with let-chains
                    let should_display_drop_destination_frame = 'scope: {
                        if !ui.rect_contains_pointer(rect) {
                            break 'scope false;
                        }

                        let Some(view_blueprint) = contents
                            .as_view_id()
                            .and_then(|view_id| self.blueprint.view(&view_id))
                        else {
                            break 'scope false;
                        };

                        let Some(dragged_payload) = dragged_payload else {
                            break 'scope false;
                        };

                        Self::handle_drop_entities_to_view(ctx, view_blueprint, dragged_payload)
                    };

                    let stroke = if should_display_drop_destination_frame {
                        tokens.drop_target_container_stroke
                    } else if hovered {
                        ui.ctx().hover_stroke()
                    } else if selected {
                        ui.ctx().selection_stroke()
                    } else {
                        continue;
                    };

                    if matches!(contents, Contents::View(_))
                        && !should_display_drop_destination_frame
                    {
                        // We already light up the view tab title; that is enough
                        continue;
                    }

                    // We want the rectangle to be on top of everything in the viewport,
                    // including stuff in "zoom-pan areas", like we use in the graph view.
                    let top_layer_id =
                        egui::LayerId::new(ui.layer_id().order, ui.id().with("child_id"));
                    ui.ctx().set_sublayer(ui.layer_id(), top_layer_id); // Make sure it is directly on top of the ui layer

                    // We paint the stroke on the inside so the panel-resize lines don't cover the highlight rectangle.
                    let painter = ui.painter().clone().with_layer_id(top_layer_id);
                    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);

                    if should_display_drop_destination_frame {
                        painter.rect_filled(
                            rect.shrink(stroke.width),
                            0.0,
                            stroke.color.gamma_multiply(0.1),
                        );
                    }
                }
            }

            if blueprint.maximized.is_none() {
                // Detect if the user has moved a tab or similar.
                // If so we can no longer automatically change the layout without discarding user edits.
                let is_dragging_a_tile = tree.dragged_id(ui.ctx()).is_some();
                if egui_tiles_delegate.edited || is_dragging_a_tile {
                    if blueprint.auto_layout() {
                        re_log::trace!(
                            "The user is manipulating the egui_tiles tree - will no longer \
                            auto-layout"
                        );
                    }

                    blueprint.set_auto_layout(false, ctx);
                }

                if egui_tiles_delegate.edited {
                    if egui_tiles_delegate.tile_dropped {
                        // Remove any empty containers left after dragging:
                        tree.simplify(&egui_tiles::SimplificationOptions {
                            prune_empty_tabs: true,
                            prune_empty_containers: false,
                            prune_single_child_tabs: true,
                            prune_single_child_containers: false,
                            all_panes_must_have_tabs: true,
                            join_nested_linear_containers: false,
                        });
                    }

                    self.blueprint
                        .deferred_commands
                        .lock()
                        .push(ViewportCommand::SetTree(tree));
                }
            }
        });

        self.blueprint.set_maximized(maximized, ctx);
    }

    /// Handle the entities being dragged over a view.
    ///
    /// Returns whether a "drop zone candidate" frame should be displayed to the user.
    ///
    /// Design decisions:
    /// - We accept the drop only if at least one of the entities is visualizable and not already
    ///   included.
    /// - When the drop happens, of all dropped entities, we only add those which are visualizable.
    ///
    fn handle_drop_entities_to_view(
        ctx: &ViewerContext<'_>,
        view_blueprint: &ViewBlueprint,
        entities: &[EntityPath],
    ) -> bool {
        let add_info = create_entity_add_info(
            ctx,
            ctx.recording().tree(),
            view_blueprint,
            ctx.lookup_query_result(view_blueprint.id),
        );

        // check if any entity or its children are visualizable and not yet included in the view
        let can_entity_be_added = |entity: &EntityPath| {
            add_info
                .get(entity)
                .is_some_and(|info| info.can_add_self_or_descendant.is_compatible_and_missing())
        };

        let any_is_visualizable = entities.iter().any(can_entity_be_added);

        ctx.drag_and_drop_manager
            .set_feedback(if any_is_visualizable {
                DragAndDropFeedback::Accept
            } else {
                DragAndDropFeedback::Reject
            });

        if !any_is_visualizable {
            return false;
        }

        // drop incoming!
        if ctx.egui_ctx().input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ctx.egui_ctx());

            view_blueprint
                .contents
                .mutate_entity_path_filter(ctx, |filter| {
                    for entity in entities {
                        if can_entity_be_added(entity) {
                            filter.add_rule(
                                RuleEffect::Include,
                                ResolvedEntityPathRule::including_subtree(entity),
                            );
                        }
                    }
                });

            ctx.selection_state()
                .set_selection(Item::View(view_blueprint.id));

            // drop is completed, no need for highlighting anymore
            false
        } else {
            any_is_visualizable
        }
    }

    pub fn on_frame_start(&self, ctx: &ViewerContext<'_>) {
        re_tracing::profile_function!();

        self.blueprint.spawn_heuristic_views(ctx);
    }

    pub fn save_to_blueprint_store(self, ctx: &ViewerContext<'_>) {
        self.blueprint.save_to_blueprint_store(ctx);
    }
}

// ----------------------------------------------------------------------------

/// `egui_tiles` has _tiles_ which are either _containers_ or _panes_.
///
/// In our case, each pane is a view,
/// while containers are just groups of things.
struct TilesDelegate<'a, 'b> {
    view_states: &'a mut ViewStates,
    ctx: &'a ViewerContext<'b>,
    viewport_blueprint: &'a ViewportBlueprint,
    maximized: &'a mut Option<ViewId>,

    /// List of query & system execution results for each view.
    executed_systems_per_view: HashMap<ViewId, (ViewQuery<'a>, SystemExecutionOutput)>,

    /// List of contents for each tile id
    contents_per_tile_id: HashMap<egui_tiles::TileId, Contents>,

    /// The user edited the tree.
    edited: bool,

    /// The user edited the tree by drag-dropping a tile.
    tile_dropped: bool,
}

impl<'a> egui_tiles::Behavior<ViewId> for TilesDelegate<'a, '_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        view_id: &mut ViewId,
    ) -> egui_tiles::UiResponse {
        re_tracing::profile_function!();

        let Some(view_blueprint) = self.viewport_blueprint.view(view_id) else {
            return Default::default();
        };

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport || !ui.is_visible() {
            return Default::default();
        }

        let (query, system_output) = self.executed_systems_per_view.remove(view_id).unwrap_or_else(|| {
            // The view's systems haven't been executed.
            // This may indicate that the egui_tiles tree is not in sync
            // with the blueprint tree.
            // This shouldn't happen, but better safe than sorry:
            // TODO(#4433): This should go to analytics

            if cfg!(debug_assertions) {
                re_log::warn_once!(
                    "Visualizers for view {:?} haven't been executed prior to display. This should never happen, please report a bug.",
                    view_blueprint.display_name_or_default()
                );
            }

            let ctx: &'a ViewerContext<'_> = self.ctx;
            let view = view_blueprint;
            re_tracing::profile_scope!("late-system-execute", view.class_identifier().as_str());

            let query_result = ctx.lookup_query_result(view.id);

            let mut per_visualizer_data_results = re_viewer_context::PerSystemDataResults::default();

            {
                re_tracing::profile_scope!("per_system_data_results");

                query_result.tree.visit(&mut |node| {
                    for system in &node.data_result.visualizers {
                        per_visualizer_data_results
                            .entry(*system)
                            .or_default()
                            .push(&node.data_result);
                    }
                    true
                });
            }

            let class = view_blueprint.class(self.ctx.view_class_registry());
            execute_systems_for_view(ctx, view, self.view_states.get_mut_or_create(*view_id, class))
        });

        let class = view_blueprint.class(self.ctx.view_class_registry());
        let view_state = self.view_states.get_mut_or_create(*view_id, class);

        ui.scope(|ui| {
            class
                .ui(self.ctx, ui, view_state, &query, system_output)
                .unwrap_or_else(|err| {
                    re_log::error!(
                        "Error in view UI (class: {}, display name: {}): {err}",
                        view_blueprint.class_identifier(),
                        class.display_name(),
                    );
                });

            ui.ctx().memory_mut(|mem| {
                mem.caches
                    .cache::<re_viewer_context::ViewRectPublisher>()
                    .set(
                        *view_id,
                        PublishedViewInfo {
                            name: view_blueprint.display_name_or_default().as_ref().to_owned(),
                            rect: ui.max_rect(),
                        },
                    );
            });
        });

        Default::default()
    }

    fn tab_title_for_pane(&mut self, view_id: &ViewId) -> egui::WidgetText {
        if let Some(view) = self.viewport_blueprint.view(view_id) {
            // Note: the formatting for unnamed views is handled by `TabWidget::new()`
            view.display_name_or_default().as_ref().into()
        } else {
            // All panes are views, so this shouldn't happen unless we have a bug
            re_log::warn_once!("ViewId missing during egui_tiles");
            self.ctx.egui_ctx().error_text("Internal error").into()
        }
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn tab_ui(
        &mut self,
        tiles: &mut egui_tiles::Tiles<ViewId>,
        ui: &mut egui::Ui,
        id: egui::Id,
        tile_id: egui_tiles::TileId,
        tab_state: &egui_tiles::TabState,
    ) -> egui::Response {
        let tab_widget = TabWidget::new(self, ui, tiles, tile_id, tab_state, 1.0);

        let response = ui
            .interact(tab_widget.rect, id, egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab);

        // Show a gap when dragged
        if ui.is_rect_visible(tab_widget.rect) && !tab_state.is_being_dragged {
            tab_widget.paint(ui);
        }

        let item = tiles.get(tile_id).and_then(|tile| match tile {
            egui_tiles::Tile::Pane(view_id) => Some(Item::View(*view_id)),

            egui_tiles::Tile::Container(_) => {
                if let Some(Contents::Container(container_id)) =
                    self.contents_per_tile_id.get(&tile_id)
                {
                    Some(Item::Container(*container_id))
                } else {
                    None
                }
            }
        });

        if let Some(item) = item {
            context_menu_ui_for_item(
                self.ctx,
                self.viewport_blueprint,
                &item,
                &response,
                SelectionUpdateBehavior::OverrideSelection,
            );
            self.ctx
                .handle_select_hover_drag_interactions(&response, item, false);
        }

        response
    }

    fn drag_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<ViewId>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
    ) {
        let tab_widget = TabWidget::new(
            self,
            ui,
            tiles,
            tile_id,
            &egui_tiles::TabState {
                active: true,
                is_being_dragged: true,
                ..Default::default()
            },
            0.5,
        );

        let frame = egui::Frame::NONE;

        frame.show(ui, |ui| {
            tab_widget.paint(ui);
        });
    }

    fn retain_pane(&mut self, view_id: &ViewId) -> bool {
        self.viewport_blueprint.views.contains_key(view_id)
    }

    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<ViewId>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let Some(active) = tabs.active.and_then(|active| tiles.get(active)) else {
            return;
        };
        let egui_tiles::Tile::Pane(view_id) = active else {
            return;
        };
        let view_id = *view_id;

        let Some(view_blueprint) = self.viewport_blueprint.view(&view_id) else {
            return;
        };
        let num_views = tiles.tiles().filter(|tile| tile.is_pane()).count();

        ui.add_space(8.0); // margin within the frame

        if *self.maximized == Some(view_id) {
            // Show minimize-button:
            if ui
                .small_icon_button(&re_ui::icons::MINIMIZE, "Restore viewport")
                .on_hover_ui(|ui| {
                    Help::new_without_title()
                        .control(
                            "Restore - show all spaces",
                            IconText::from_keyboard_shortcut(ui.ctx().os(), TOGGLE_MAXIMIZE_VIEW),
                        )
                        .ui(ui);
                })
                .clicked()
                || ui.input_mut(|input| input.consume_shortcut(&TOGGLE_MAXIMIZE_VIEW))
            {
                *self.maximized = None;
                MaximizeAnimationState::restore_view(ui.ctx(), view_id);
            }
        } else if num_views > 1 {
            // Show maximize-button:
            let is_view_the_only_selected =
                self.ctx.selection().is_view_the_only_selected(&view_id);
            let toggle = is_view_the_only_selected
                && ui.input_mut(|input| input.consume_shortcut(&TOGGLE_MAXIMIZE_VIEW));
            if ui
                .small_icon_button(&re_ui::icons::MAXIMIZE, "Maximize view")
                .on_hover_ui(|ui| {
                    if is_view_the_only_selected {
                        Help::new_without_title()
                            .control(
                                "Maximize view",
                                IconText::from_keyboard_shortcut(
                                    ui.ctx().os(),
                                    TOGGLE_MAXIMIZE_VIEW,
                                ),
                            )
                            .ui(ui);
                    } else {
                        ui.label("Maximize view");
                    }
                })
                .clicked()
                || toggle
            {
                // Just maximize - don't select. See https://github.com/rerun-io/rerun/issues/2861
                *self.maximized = Some(view_id);

                if let Some(rect) = tiles.rect(tile_id) {
                    MaximizeAnimationState::start_maximizing(ui.ctx(), view_id, rect);
                }
            }
        }

        let view_class = view_blueprint.class(self.ctx.view_class_registry());

        // give the view a chance to display some extra UI in the top bar.
        let view_state = self.view_states.get_mut_or_create(view_id, view_class);
        view_class
            .extra_title_bar_ui(
                self.ctx,
                ui,
                view_state,
                &view_blueprint.space_origin,
                view_id,
            )
            .unwrap_or_else(|err| {
                re_log::error!(
                    "Error in view title bar UI (class: {}, display name: {}): {err}",
                    view_blueprint.class_identifier(),
                    view_class.display_name(),
                );
            });

        ui.help_button(|ui| {
            view_class.help(ui.ctx().os()).ui(ui);
        });
    }

    // Styling:

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        let theme = if visuals.dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        re_ui::design_tokens_of(theme).tab_bar_color
    }

    fn dragged_overlay_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        visuals.panel_fill.gamma_multiply(0.5)
    }

    /// When drag-and-dropping a tile, the candidate area is drawn with this stroke.
    fn drag_preview_stroke(&self, visuals: &egui::Visuals) -> egui::Stroke {
        design_tokens_of_visuals(visuals).tile_drag_preview_stroke
    }

    /// When drag-and-dropping a tile, the candidate area is drawn with this background color.
    fn drag_preview_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        design_tokens_of_visuals(visuals).tile_drag_preview_color
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, style: &egui::Style) -> f32 {
        re_ui::design_tokens_of_visuals(&style.visuals).title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    ///
    /// These options are applied on every frame by `egui_tiles`.
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        re_viewport_blueprint::tree_simplification_options()
    }

    // Callbacks:

    fn on_edit(&mut self, edit_action: egui_tiles::EditAction) {
        re_log::trace!("Tree edit: {edit_action:?}");
        match edit_action {
            EditAction::TileDropped => {
                self.tile_dropped = true;
                self.edited = true;
            }

            EditAction::TabSelected | EditAction::TileResized => {
                self.edited = true;
            }
            EditAction::TileDragged => {
                // No synchronization needed, because TileDragged happens when a drag starts, so no tiles are actually
                // modified. When the drag completes, then we get `TileDropped` and run the synchronization.
            }
        }
    }
}

/// A tab button for a tab in the viewport.
///
/// The tab can contain any `egui_tiles::Tile`,
/// which is either a Pane with a View, or a container,
/// e.g. a grid of tiles.
struct TabWidget {
    galley: std::sync::Arc<egui::Galley>,
    rect: egui::Rect,
    galley_rect: egui::Rect,
    icon: &'static Icon,
    icon_size: egui::Vec2,
    icon_rect: egui::Rect,
    bg_color: egui::Color32,
    text_color: egui::Color32,
    unnamed_style: bool,
}

impl TabWidget {
    fn new<'a>(
        tab_viewer: &'a mut TilesDelegate<'_, '_>,
        ui: &'a mut egui::Ui,
        tiles: &'a egui_tiles::Tiles<ViewId>,
        tile_id: egui_tiles::TileId,
        tab_state: &egui_tiles::TabState,
        gamma: f32,
    ) -> Self {
        let tokens = ui.tokens();

        struct TabDesc {
            label: egui::WidgetText,
            user_named: bool,
            icon: &'static re_ui::Icon,
            item: Option<Item>,
        }

        let tab_desc = match tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(view_id)) => {
                if let Some(view) = tab_viewer.viewport_blueprint.view(view_id) {
                    TabDesc {
                        label: tab_viewer.tab_title_for_pane(view_id),
                        user_named: view.display_name.is_some(),
                        icon: view.class(tab_viewer.ctx.view_class_registry()).icon(),
                        item: Some(Item::View(*view_id)),
                    }
                } else {
                    re_log::warn_once!("View {view_id} not found");

                    TabDesc {
                        label: tab_viewer.ctx.egui_ctx().error_text("Unknown view").into(),
                        icon: &re_ui::icons::VIEW_GENERIC,
                        user_named: false,
                        item: None,
                    }
                }
            }
            Some(egui_tiles::Tile::Container(container)) => {
                if let Some(Contents::Container(container_id)) =
                    tab_viewer.contents_per_tile_id.get(&tile_id)
                {
                    let (label, user_named) = if let Some(container_blueprint) =
                        tab_viewer.viewport_blueprint.container(container_id)
                    {
                        (
                            container_blueprint
                                .display_name_or_default()
                                .as_ref()
                                .into(),
                            container_blueprint.display_name.is_some(),
                        )
                    } else {
                        re_log::warn_once!("Container {container_id} missing during egui_tiles");
                        (
                            tab_viewer
                                .ctx
                                .egui_ctx()
                                .error_text("Internal error")
                                .into(),
                            false,
                        )
                    };

                    TabDesc {
                        label,
                        user_named,
                        icon: icon_for_container_kind(&container.kind()),
                        item: Some(Item::Container(*container_id)),
                    }
                } else {
                    // If the container is a tab with a single child, we can display the child's name instead. This
                    // fallback is required because, often, single-child tabs were autogenerated by egui_tiles and do
                    // not have a matching ContainerBlueprint.
                    if container.kind() == egui_tiles::ContainerKind::Tabs {
                        if let Some(tile_id) = container.only_child() {
                            return Self::new(tab_viewer, ui, tiles, tile_id, tab_state, gamma);
                        }
                    }

                    re_log::warn_once!("Container for tile ID {tile_id:?} not found");

                    TabDesc {
                        label: tab_viewer
                            .ctx
                            .egui_ctx()
                            .error_text("Unknown container")
                            .into(),
                        icon: &re_ui::icons::VIEW_GENERIC,
                        user_named: false,
                        item: None,
                    }
                }
            }
            None => {
                re_log::warn_once!("Tile {tile_id:?} not found");

                TabDesc {
                    label: tab_viewer
                        .ctx
                        .egui_ctx()
                        .error_text("Internal error")
                        .into(),
                    icon: &re_ui::icons::VIEW_UNKNOWN,
                    user_named: false,
                    item: None,
                }
            }
        };

        let hovered = tab_desc
            .item
            .as_ref()
            .is_some_and(|item| tab_viewer.ctx.hovered().contains_item(item));
        let selected = tab_desc
            .item
            .as_ref()
            .is_some_and(|item| tab_viewer.ctx.selection().contains_item(item));

        // tab icon
        let icon_size = tokens.small_icon_size;
        let icon_width_plus_padding = icon_size.x + tokens.text_to_icon_padding();

        // tab title
        let text = if !tab_desc.user_named {
            //TODO(ab): use design tokens
            tab_desc.label.italics()
        } else {
            tab_desc.label
        };

        let font_id = egui::TextStyle::Button.resolve(ui.style());
        let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, font_id);

        let x_margin = tab_viewer.tab_title_spacing(ui.visuals());
        let (_, rect) = ui.allocate_space(egui::vec2(
            galley.size().x + 2.0 * x_margin + icon_width_plus_padding,
            tokens.title_bar_height(),
        ));
        let galley_rect = egui::Rect::from_two_pos(
            rect.min + egui::vec2(icon_width_plus_padding, 0.0),
            rect.max,
        );
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + x_margin + icon_size.x / 2.0, rect.center().y),
            icon_size,
        );

        let bg_color = if selected {
            ui.visuals().selection.bg_fill
        } else if hovered {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            tab_viewer.tab_bar_color(ui.visuals())
        };
        let bg_color = bg_color.gamma_multiply(gamma);
        let text_color = tab_viewer
            .tab_text_color(ui.visuals(), tiles, tile_id, tab_state)
            .gamma_multiply(gamma);

        Self {
            galley,
            rect,
            galley_rect,
            icon: tab_desc.icon,
            icon_size,
            icon_rect,
            bg_color,
            text_color,
            unnamed_style: !tab_desc.user_named,
        }
    }

    fn paint(self, ui: &egui::Ui) {
        ui.painter().rect_filled(self.rect, 0.0, self.bg_color);

        let icon_image = self
            .icon
            .as_image()
            .fit_to_exact_size(self.icon_size)
            .tint(self.text_color);
        icon_image.paint_at(ui, self.icon_rect);

        //TODO(ab): use design tokens
        let label_color = if self.unnamed_style {
            self.text_color.gamma_multiply(0.5)
        } else {
            self.text_color
        };

        ui.painter().galley(
            egui::Align2::CENTER_CENTER
                .align_size_within_rect(self.galley.size(), self.galley_rect)
                .min,
            self.galley,
            label_color,
        );
    }
}

// ----------------------------------------------------------------------------

/// This enables best-effort animation when one maximizes/restores a view.
///
/// There are a few ways this can fail (gracefully):
/// - Maximization happens outside of this file (I don't think we have a way of doing that atm though).
/// - The viewport has changes chaped since we last maximized
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum MaximizeAnimationState {
    #[default]
    Nothing,

    /// We are in the progress of maximizing, or have finished doing so.
    ///
    /// We keep this around even after we've finished, because
    /// we use this to remember the pre-maxmize rect of the view,
    /// and _hope_ that it's where it will return to.
    /// This might cause visual glitches if the viewport has changed shape since we maximized.
    Maximizing {
        /// What view is being maximized?
        view_id: ViewId,

        /// When maximization started.
        start_time: web_time::Instant,

        /// Where the view started.
        normal_rect: egui::Rect,
    },

    Restoring {
        /// What view is being restored?
        view_id: ViewId,

        /// When restoration started.
        start_time: web_time::Instant,

        /// Where the view should end up.
        normal_rect: egui::Rect,
    },
}

impl MaximizeAnimationState {
    fn start_maximizing(egui_ctx: &egui::Context, view_id: ViewId, rect: egui::Rect) {
        // Animate the maximization of the view:
        egui_ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::NULL,
                Self::Maximizing {
                    view_id,
                    normal_rect: rect,
                    start_time: web_time::Instant::now(),
                },
            );
        });
        egui_ctx.request_repaint();
    }

    fn restore_view(egui_ctx: &egui::Context, view_id: ViewId) {
        egui_ctx.data_mut(|data| {
            let animation_state = data.get_temp_mut_or_default(egui::Id::NULL);

            if let Self::Maximizing {
                view_id: animation_view_id,
                normal_rect,
                ..
            } = animation_state
            {
                if view_id == *animation_view_id {
                    // We can do a restoration animation!
                    *animation_state = Self::Restoring {
                        view_id,
                        start_time: web_time::Instant::now(),
                        normal_rect: *normal_rect,
                    };
                }
            }
        });
        egui_ctx.request_repaint();
    }

    fn animated_view_and_rect(
        self,
        egui_ctx: &egui::Context,
        viewport_rect: egui::Rect,
    ) -> (Option<ViewId>, egui::Rect) {
        let animation_time = egui_ctx.style().animation_time;

        let mut animating_view_id = None;
        let mut animated_rect = viewport_rect;

        match self {
            Self::Nothing => {}

            Self::Maximizing {
                view_id,
                start_time,
                normal_rect,
            } => {
                // Animate the maximization of the view:
                let progress = remap_clamp(
                    start_time.elapsed().as_secs_f32(),
                    0.0..=animation_time,
                    0.0..=1.0,
                );
                let progress = egui::emath::easing::quadratic_out(progress); // Move quickly at first, then slow down

                if progress < 1.0 {
                    egui_ctx.request_repaint();
                    animated_rect = normal_rect.lerp_towards(&viewport_rect, progress);
                    animating_view_id = Some(view_id);
                } else {
                    // Keep the Maximizing state so we remember the pre-maximized rect
                }
            }

            Self::Restoring {
                view_id,
                start_time,
                normal_rect,
            } => {
                // Animate the restoring of the view:
                let progress = remap_clamp(
                    start_time.elapsed().as_secs_f32(),
                    0.0..=animation_time,
                    0.0..=1.0,
                );
                let progress = egui::emath::easing::quadratic_out(progress); // Move quickly at first, then slow down
                if progress < 1.0 {
                    egui_ctx.request_repaint();
                    animated_rect = viewport_rect.lerp_towards(&normal_rect, progress);
                    animating_view_id = Some(view_id);
                }
            }
        };

        // Prevent glitches when the viewport has changed size since the animation started.
        let animated_rect = animated_rect.intersect(viewport_rect);

        (animating_view_id, animated_rect)
    }
}
