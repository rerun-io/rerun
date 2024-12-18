//! The viewport panel.
//!
//! Contains all views.

use ahash::HashMap;
use egui_tiles::{Behavior as _, EditAction};

use re_context_menu::{context_menu_ui_for_item, SelectionUpdateBehavior};
use re_log_types::{EntityPath, EntityPathRule, RuleEffect};
use re_ui::{design_tokens, ContextExt as _, DesignTokens, Icon, UiExt as _};
use re_viewer_context::{
    blueprint_id_to_tile_id, icon_for_container_kind, Contents, DragAndDropFeedback,
    DragAndDropPayload, Item, PublishedViewInfo, SystemExecutionOutput, ViewClassRegistry, ViewId,
    ViewQuery, ViewStates, ViewerContext,
};
use re_viewport_blueprint::{
    create_entity_add_info, ViewBlueprint, ViewportBlueprint, ViewportCommand,
};

use crate::system_execution::{execute_systems_for_all_views, execute_systems_for_view};

fn tree_simplification_options() -> egui_tiles::SimplificationOptions {
    egui_tiles::SimplificationOptions {
        prune_empty_tabs: false,
        all_panes_must_have_tabs: true,
        prune_empty_containers: false,
        prune_single_child_tabs: false,
        prune_single_child_containers: false,
        join_nested_linear_containers: true,
    }
}

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

        let mut tree = if let Some(view_id) = blueprint.maximized {
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
            ui.spacing_mut().item_spacing.x = DesignTokens::view_padding();

            re_tracing::profile_scope!("tree.ui");

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
                        design_tokens().drop_target_container_stroke()
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

                    // We need to shrink a bit so the panel-resize lines don't cover the highlight rectangle.
                    // This is hacky.
                    let painter = ui.painter().clone().with_layer_id(top_layer_id);
                    painter.rect_stroke(rect.shrink(stroke.width), 0.0, stroke);

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
        if ctx.egui_ctx.input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ctx.egui_ctx);

            view_blueprint
                .contents
                .mutate_entity_path_filter(ctx, |filter| {
                    for entity in entities {
                        if can_entity_be_added(entity) {
                            filter.add_rule(
                                RuleEffect::Include,
                                EntityPathRule::including_subtree(entity.clone()),
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

    /// Process any deferred [`ViewportCommand`] and then save to blueprint store (if needed).
    pub fn save_to_blueprint_store(
        self,
        ctx: &ViewerContext<'_>,
        view_class_registry: &ViewClassRegistry,
    ) {
        re_tracing::profile_function!();

        let Self { mut blueprint } = self;

        let commands: Vec<ViewportCommand> = blueprint.deferred_commands.lock().drain(..).collect();

        if commands.is_empty() {
            return; // No changes this frame - no need to save to blueprint store.
        }

        let mut run_auto_layout = false;

        for command in commands {
            apply_viewport_command(ctx, &mut blueprint, command, &mut run_auto_layout);
        }

        if run_auto_layout {
            blueprint.tree =
                super::auto_layout::tree_from_views(view_class_registry, &blueprint.views);
        }

        // Simplify before we save the tree.
        // `egui_tiles` also runs a simplifying pass when calling `tree.ui`, but that is too late.
        // We want the simplified changes saved to the store:
        blueprint.tree.simplify(&tree_simplification_options());

        // TODO(emilk): consider diffing the tree against the state it was in at the start of the frame,
        // so that we only save it if it actually changed.

        blueprint.save_tree_as_containers(ctx);
    }
}

fn apply_viewport_command(
    ctx: &ViewerContext<'_>,
    bp: &mut ViewportBlueprint,
    command: ViewportCommand,
    run_auto_layout: &mut bool,
) {
    re_log::trace!("Processing viewport command: {command:?}");
    match command {
        ViewportCommand::SetTree(new_tree) => {
            bp.tree = new_tree;
        }

        ViewportCommand::AddView {
            view,
            parent_container,
            position_in_parent,
        } => {
            let view_id = view.id;

            view.save_to_blueprint_store(ctx);
            bp.views.insert(view_id, view);

            if bp.auto_layout() {
                // No need to add to the tree - we'll create a new tree from scratch instead.
                re_log::trace!(
                    "Running auto-layout after adding a view because auto_layout is turned on"
                );
                *run_auto_layout = true;
            } else {
                // Add the view to the tree:
                let parent_id = parent_container.unwrap_or(bp.root_container);
                re_log::trace!("Adding view {view_id} to parent {parent_id}");
                let tile_id = bp.tree.tiles.insert_pane(view_id);
                let container_tile_id = blueprint_id_to_tile_id(&parent_id);
                if let Some(egui_tiles::Tile::Container(container)) =
                    bp.tree.tiles.get_mut(container_tile_id)
                {
                    re_log::trace!("Inserting new view into root container");
                    container.add_child(tile_id);
                    if let Some(position_in_parent) = position_in_parent {
                        bp.tree.move_tile_to_container(
                            tile_id,
                            container_tile_id,
                            position_in_parent,
                            true,
                        );
                    }
                } else {
                    re_log::trace!(
                        "Parent was not a container (or not found) - will re-run auto-layout"
                    );
                    *run_auto_layout = true;
                }
            }
        }

        ViewportCommand::AddContainer {
            container_kind,
            parent_container,
        } => {
            let parent_id = parent_container.unwrap_or(bp.root_container);

            let tile_id = bp
                .tree
                .tiles
                .insert_container(egui_tiles::Container::new(container_kind, vec![]));

            re_log::trace!("Adding container {container_kind:?} to parent {parent_id}");

            if let Some(egui_tiles::Tile::Container(parent_container)) =
                bp.tree.tiles.get_mut(blueprint_id_to_tile_id(&parent_id))
            {
                re_log::trace!("Inserting new view into container {parent_id:?}");
                parent_container.add_child(tile_id);
            } else {
                re_log::trace!("Parent or root was not a container - will re-run auto-layout");
                *run_auto_layout = true;
            }
        }

        ViewportCommand::SetContainerKind(container_id, container_kind) => {
            if let Some(egui_tiles::Tile::Container(container)) = bp
                .tree
                .tiles
                .get_mut(blueprint_id_to_tile_id(&container_id))
            {
                re_log::trace!("Mutating container {container_id:?} to {container_kind:?}");
                container.set_kind(container_kind);
            } else {
                re_log::trace!("No root found - will re-run auto-layout");
            }
        }

        ViewportCommand::FocusTab(view_id) => {
            let found = bp.tree.make_active(|_, tile| match tile {
                egui_tiles::Tile::Pane(this_view_id) => *this_view_id == view_id,
                egui_tiles::Tile::Container(_) => false,
            });
            re_log::trace!("Found tab to focus on for view ID {view_id}: {found}");
        }

        ViewportCommand::RemoveContents(contents) => {
            let tile_id = contents.as_tile_id();

            for tile in bp.tree.remove_recursively(tile_id) {
                re_log::trace!("Removing tile {tile_id:?}");
                match tile {
                    egui_tiles::Tile::Pane(view_id) => {
                        re_log::trace!("Removing view {view_id}");

                        // Remove the view from the store
                        if let Some(view) = bp.views.get(&view_id) {
                            view.clear(ctx);
                        }

                        // If the view was maximized, clean it up
                        if bp.maximized == Some(view_id) {
                            bp.set_maximized(None, ctx);
                        }

                        bp.views.remove(&view_id);
                    }
                    egui_tiles::Tile::Container(_) => {
                        // Empty containers (like this one) will be auto-removed by the tree simplification algorithm,
                        // that will run later because of this tree edit.
                    }
                }
            }

            bp.mark_user_interaction(ctx);

            if Some(tile_id) == bp.tree.root {
                bp.tree.root = None;
            }
        }

        ViewportCommand::SimplifyContainer(container_id, options) => {
            re_log::trace!("Simplifying tree with options: {options:?}");
            let tile_id = blueprint_id_to_tile_id(&container_id);
            bp.tree.simplify_children_of_tile(tile_id, &options);
        }

        ViewportCommand::MakeAllChildrenSameSize(container_id) => {
            let tile_id = blueprint_id_to_tile_id(&container_id);
            if let Some(egui_tiles::Tile::Container(container)) = bp.tree.tiles.get_mut(tile_id) {
                match container {
                    egui_tiles::Container::Tabs(_) => {}
                    egui_tiles::Container::Linear(linear) => {
                        linear.shares = Default::default();
                    }
                    egui_tiles::Container::Grid(grid) => {
                        grid.col_shares = Default::default();
                        grid.row_shares = Default::default();
                    }
                }
            }
        }

        ViewportCommand::MoveContents {
            contents_to_move,
            target_container,
            target_position_in_container,
        } => {
            re_log::trace!(
                "Moving {contents_to_move:?} to container {target_container:?} at pos \
                        {target_position_in_container}"
            );

            // TODO(ab): the `rev()` is better preserve ordering when moving a group of items. There
            // remains some ordering (and possibly insertion point error) edge cases when dragging
            // multiple item within the same container. This should be addressed by egui_tiles:
            // https://github.com/rerun-io/egui_tiles/issues/90
            for contents in contents_to_move.iter().rev() {
                let contents_tile_id = contents.as_tile_id();
                let target_container_tile_id = blueprint_id_to_tile_id(&target_container);

                bp.tree.move_tile_to_container(
                    contents_tile_id,
                    target_container_tile_id,
                    target_position_in_container,
                    true,
                );
            }
        }

        ViewportCommand::MoveContentsToNewContainer {
            contents_to_move,
            new_container_kind,
            target_container,
            target_position_in_container,
        } => {
            let new_container_tile_id = bp
                .tree
                .tiles
                .insert_container(egui_tiles::Container::new(new_container_kind, vec![]));

            let target_container_tile_id = blueprint_id_to_tile_id(&target_container);
            bp.tree.move_tile_to_container(
                new_container_tile_id,
                target_container_tile_id,
                target_position_in_container,
                true, // reflow grid if needed
            );

            for (pos, content) in contents_to_move.into_iter().enumerate() {
                bp.tree.move_tile_to_container(
                    content.as_tile_id(),
                    new_container_tile_id,
                    pos,
                    true, // reflow grid if needed
                );
            }
        }
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

        let Some(latest_at) = self.ctx.rec_cfg.time_ctrl.read().time_int() else {
            ui.centered_and_justified(|ui| {
                ui.weak("No time selected");
            });
            return Default::default();
        };

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

            let class = view_blueprint.class(self.ctx.view_class_registry);
            execute_systems_for_view(ctx, view, latest_at, self.view_states.get_mut_or_create(*view_id, class))
        });

        let class = view_blueprint.class(self.ctx.view_class_registry);
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
            self.ctx.egui_ctx.error_text("Internal error").into()
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

        let frame = egui::Frame {
            inner_margin: egui::Margin::same(0.),
            outer_margin: egui::Margin::same(0.),
            rounding: egui::Rounding::ZERO,
            shadow: Default::default(),
            fill: egui::Color32::TRANSPARENT,
            stroke: egui::Stroke::NONE,
        };

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
        _tile_id: egui_tiles::TileId,
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
                .small_icon_button(&re_ui::icons::MINIMIZE)
                .on_hover_text("Restore - show all spaces")
                .clicked()
            {
                *self.maximized = None;
            }
        } else if num_views > 1 {
            // Show maximize-button:
            if ui
                .small_icon_button(&re_ui::icons::MAXIMIZE)
                .on_hover_text("Maximize view")
                .clicked()
            {
                *self.maximized = Some(view_id);
                // Just maximize - don't select. See https://github.com/rerun-io/rerun/issues/2861
            }
        }

        let view_class = view_blueprint.class(self.ctx.view_class_registry);

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

        let help_markdown = view_class.help_markdown(self.ctx.egui_ctx);
        ui.help_hover_button().on_hover_ui(|ui| {
            ui.markdown_ui(&help_markdown);
        });
    }

    // Styling:

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        re_ui::design_tokens().tab_bar_color
    }

    fn dragged_overlay_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        visuals.panel_fill.gamma_multiply(0.5)
    }

    /// When drag-and-dropping a tile, the candidate area is drawn with this stroke.
    fn drag_preview_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, egui::Color32::WHITE.gamma_multiply(0.5))
    }

    /// When drag-and-dropping a tile, the candidate area is drawn with this background color.
    fn drag_preview_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        egui::Color32::WHITE.gamma_multiply(0.1)
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        re_ui::DesignTokens::title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    ///
    /// These options are applied on every frame by `egui_tiles`.
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        tree_simplification_options()
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
                        icon: view.class(tab_viewer.ctx.view_class_registry).icon(),
                        item: Some(Item::View(*view_id)),
                    }
                } else {
                    re_log::warn_once!("View {view_id} not found");

                    TabDesc {
                        label: tab_viewer.ctx.egui_ctx.error_text("Unknown view").into(),
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
                            tab_viewer.ctx.egui_ctx.error_text("Internal error").into(),
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
                            .egui_ctx
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
                    label: tab_viewer.ctx.egui_ctx.error_text("Internal error").into(),
                    icon: &re_ui::icons::VIEW_UNKNOWN,
                    user_named: false,
                    item: None,
                }
            }
        };

        let hovered = tab_desc
            .item
            .as_ref()
            .map_or(false, |item| tab_viewer.ctx.hovered().contains_item(item));
        let selected = tab_desc
            .item
            .as_ref()
            .map_or(false, |item| tab_viewer.ctx.selection().contains_item(item));

        // tab icon
        let icon_size = DesignTokens::small_icon_size();
        let icon_width_plus_padding = icon_size.x + DesignTokens::text_to_icon_padding();

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
            DesignTokens::title_bar_height(),
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
        ui.painter()
            .rect(self.rect, 0.0, self.bg_color, egui::Stroke::NONE);

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
