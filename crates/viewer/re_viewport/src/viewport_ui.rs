//! The viewport panel.
//!
//! Contains all space views.

use std::sync::Arc;

use ahash::HashMap;
use egui_tiles::{Behavior as _, EditAction};
use parking_lot::Mutex;

use re_context_menu::{context_menu_ui_for_item, SelectionUpdateBehavior};
use re_renderer::ScreenshotProcessor;
use re_types_blueprint::blueprint::components::IncludedSpaceView;
use re_ui::{ContextExt as _, DesignTokens, Icon, UiExt as _};
use re_viewer_context::{
    blueprint_id_to_tile_id, icon_for_container_kind, ContainerId, Contents, Item,
    SpaceViewClassRegistry, SpaceViewId, SystemExecutionOutput, ViewQuery, ViewStates,
    ViewerContext,
};
use re_viewport_blueprint::{ViewportBlueprint, ViewportCommand, VIEWPORT_PATH};

use crate::{
    screenshot::handle_pending_space_view_screenshots,
    system_execution::{execute_systems_for_all_views, execute_systems_for_space_view},
};

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

        if let Some(space_view_id) = blueprint.maximized {
            if !blueprint.space_views.contains_key(&space_view_id) {
                maximized = None;
            } else if let Some(tile_id) = blueprint.tree.tiles.find_pane(&space_view_id) {
                if !blueprint.tree.tiles.is_visible(tile_id) {
                    maximized = None;
                }
            }
        }

        let mut tree = if let Some(space_view_id) = blueprint.maximized {
            let mut tiles = egui_tiles::Tiles::default();
            let root = tiles.insert_pane(space_view_id);
            egui_tiles::Tree::new("viewport_tree", root, tiles)
        } else {
            blueprint.tree.clone()
        };

        let executed_systems_per_space_view =
            execute_systems_for_all_views(ctx, &tree, &blueprint.space_views, view_states);

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
                edited: false,
                executed_systems_per_space_view,
                contents_per_tile_id,
                deferred_commands: self.blueprint.deferred_commands.clone(),
                root_container_id: self.blueprint.root_container,
            };

            tree.ui(&mut egui_tiles_delegate, ui);

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

                    let stroke = if hovered {
                        ui.ctx().hover_stroke()
                    } else if selected {
                        ui.ctx().selection_stroke()
                    } else {
                        continue;
                    };

                    ui.painter()
                        .rect_stroke(rect.shrink(stroke.width / 2.0), 0.0, stroke);
                }
            }

            if blueprint.maximized.is_none() {
                // Detect if the user has moved a tab or similar.
                // If so we can no longer automatically change the layout without discarding user edits.
                let is_dragging_a_tile = tree.dragged_id(ui.ctx()).is_some();
                if egui_tiles_delegate.edited || is_dragging_a_tile {
                    if blueprint.auto_layout() {
                        re_log::trace!(
                            "The user is manipulating the egui_tiles tree - will no longer auto-layout"
                        );
                    }

                    blueprint.set_auto_layout(false, ctx);
                }

                if egui_tiles_delegate.edited {
                    // TODO(#4687): Be extra careful here. If we mark edited inappropriately we can create an infinite edit loop.
                    self.blueprint.deferred_commands.lock().push(ViewportCommand::SetTree(tree));
                }
            }
        });

        self.blueprint.set_maximized(maximized, ctx);
    }

    pub fn on_frame_start(&self, ctx: &ViewerContext<'_>) {
        re_tracing::profile_function!();

        // Handle pending view screenshots:
        if let Some(render_ctx) = ctx.render_ctx {
            for space_view in self.blueprint.space_views.values() {
                #[allow(clippy::blocks_in_conditions)]
                while ScreenshotProcessor::next_readback_result(
                    render_ctx,
                    space_view.id.gpu_readback_id(),
                    |data, extent, mode| {
                        handle_pending_space_view_screenshots(space_view, data, extent, mode);
                    },
                )
                .is_some()
                {}
            }
        }

        self.blueprint.spawn_heuristic_space_views(ctx);
    }

    /// Process any deferred `TreeActions` and then sync to blueprint
    // TODO: move to ViewportBlueprint
    pub fn update_and_sync_tile_tree_to_blueprint(
        self,
        ctx: &ViewerContext<'_>,
        space_view_class_registry: &SpaceViewClassRegistry,
    ) {
        re_tracing::profile_function!();
        // At the end of the Tree-UI, we can safely apply deferred actions.

        let Self { blueprint: mut bp } = self;

        let mut run_auto_layout = false;

        // TODO(#4687): Be extra careful here. If we mark edited inappropriately we can create an infinite edit loop.
        let commands: Vec<ViewportCommand> = bp.deferred_commands.lock().drain(..).collect();

        // If set, we will save the tree to the blueprint store at the end of the frame.
        // This includes saving all the containers in the tree (but not the space views!).
        let mut tree_edited = false;

        for command in commands {
            re_log::trace!("Processing viewport command: {command:?}");
            match command {
                ViewportCommand::SetTree(new_tree) => {
                    bp.tree = new_tree;
                    tree_edited = true;
                }

                ViewportCommand::AddSpaceView {
                    space_view,
                    parent_container,
                    position_in_parent,
                } => {
                    let space_view_id = space_view.id;

                    space_view.save_to_blueprint_store(ctx);
                    bp.space_views.insert(space_view_id, space_view);

                    if bp.auto_layout() {
                        // No need to add to the tree - we'll create a new tree from scratch instead.
                        re_log::trace!("Running auto-layout after adding a space-view because auto_layout is turned on");
                        run_auto_layout = true;
                    } else {
                        // Add the view to the tree:
                        let parent_id = parent_container.unwrap_or(bp.root_container);
                        re_log::trace!("Adding space-view {space_view_id} to parent {parent_id}");
                        let tile_id = bp.tree.tiles.insert_pane(space_view_id);
                        let container_tile_id = blueprint_id_to_tile_id(&parent_id);
                        if let Some(egui_tiles::Tile::Container(container)) =
                            bp.tree.tiles.get_mut(container_tile_id)
                        {
                            re_log::trace!("Inserting new space view into root container");
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
                            re_log::trace!("Parent was not a container (or not found) - will re-run auto-layout");
                            run_auto_layout = true;
                        }
                    }

                    tree_edited = true;
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
                        re_log::trace!("Inserting new space view into container {parent_id:?}");
                        parent_container.add_child(tile_id);
                    } else {
                        re_log::trace!(
                            "Parent or root was not a container - will re-run auto-layout"
                        );
                        run_auto_layout = true;
                    }

                    tree_edited = true;
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

                    tree_edited = true;
                }

                ViewportCommand::FocusTab(space_view_id) => {
                    let found = bp.tree.make_active(|_, tile| match tile {
                        egui_tiles::Tile::Pane(this_space_view_id) => {
                            *this_space_view_id == space_view_id
                        }
                        egui_tiles::Tile::Container(_) => false,
                    });
                    re_log::trace!(
                        "Found tab to focus on for space view ID {space_view_id}: {found}"
                    );
                    tree_edited = true;
                }

                ViewportCommand::RemoveContents(contents) => {
                    let tile_id = contents.as_tile_id();

                    for tile in bp.tree.remove_recursively(tile_id) {
                        re_log::trace!("Removing tile {tile_id:?}");
                        match tile {
                            egui_tiles::Tile::Pane(space_view_id) => {
                                re_log::trace!("Removing space view {space_view_id}");

                                // Remove the space view from the store
                                if let Some(space_view) = bp.space_views.get(&space_view_id) {
                                    space_view.clear(ctx);
                                }

                                // If the space-view was maximized, clean it up
                                if bp.maximized == Some(space_view_id) {
                                    bp.set_maximized(None, ctx);
                                }

                                bp.space_views.remove(&space_view_id);

                                let components: Vec<IncludedSpaceView> = bp
                                    .space_views
                                    .keys()
                                    .map(|id| IncludedSpaceView((*id).into()))
                                    .collect();
                                ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &components);
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
                    tree_edited = true;
                }

                ViewportCommand::SimplifyContainer(container_id, options) => {
                    re_log::trace!("Simplifying tree with options: {options:?}");
                    let tile_id = blueprint_id_to_tile_id(&container_id);
                    bp.tree.simplify_children_of_tile(tile_id, &options);
                    tree_edited = true;
                }

                ViewportCommand::MakeAllChildrenSameSize(container_id) => {
                    let tile_id = blueprint_id_to_tile_id(&container_id);
                    if let Some(egui_tiles::Tile::Container(container)) =
                        bp.tree.tiles.get_mut(tile_id)
                    {
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
                    tree_edited = true;
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

                    let contents_tile_id = contents_to_move.as_tile_id();
                    let target_container_tile_id = blueprint_id_to_tile_id(&target_container);

                    bp.tree.move_tile_to_container(
                        contents_tile_id,
                        target_container_tile_id,
                        target_position_in_container,
                        true,
                    );
                    tree_edited = true;
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

                    tree_edited = true;
                }
            }
        }

        if run_auto_layout {
            bp.tree = super::auto_layout::tree_from_space_views(
                space_view_class_registry,
                &bp.space_views,
            );

            tree_edited = true;
        }

        // Finally, save any edits to the blueprint tree
        // This is a no-op if the tree hasn't changed.
        if tree_edited {
            // TODO(#4687): Be extra careful here. If we mark edited inappropriately we can create an infinite edit loop.

            // Simplify before we save the tree.
            // `egui_tiles` also runs a simplifying pass when calling `tree.ui`, but that is too late.
            // We want the simplified changes saved to the store:
            bp.tree.simplify(&tree_simplification_options());

            bp.save_tree_as_containers(ctx);
        }
    }
}

// ----------------------------------------------------------------------------

/// `egui_tiles` has _tiles_ which are either _containers_ or _panes_.
///
/// In our case, each pane is a space view,
/// while containers are just groups of things.
struct TilesDelegate<'a, 'b> {
    view_states: &'a mut ViewStates,
    ctx: &'a ViewerContext<'b>,
    viewport_blueprint: &'a ViewportBlueprint,
    maximized: &'a mut Option<SpaceViewId>,
    root_container_id: ContainerId,
    deferred_commands: Arc<Mutex<Vec<ViewportCommand>>>,

    /// List of query & system execution results for each space view.
    executed_systems_per_space_view: HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)>,

    /// List of contents for each tile id
    contents_per_tile_id: HashMap<egui_tiles::TileId, Contents>,

    /// The user edited the tree.
    edited: bool,
}

impl<'a, 'b> egui_tiles::Behavior<SpaceViewId> for TilesDelegate<'a, 'b> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        view_id: &mut SpaceViewId,
    ) -> egui_tiles::UiResponse {
        re_tracing::profile_function!();

        let Some(space_view_blueprint) = self.viewport_blueprint.view(view_id) else {
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

        let (query, system_output) = self.executed_systems_per_space_view.remove(view_id).unwrap_or_else(|| {
            // The space view's systems haven't been executed.
            // This may indicate that the egui_tiles tree is not in sync
            // with the blueprint tree.
            // This shouldn't happen, but better safe than sorry:
            // TODO(#4433): This should go to analytics

            if cfg!(debug_assertions) {
                re_log::warn_once!(
                    "Visualizers for space view {:?} haven't been executed prior to display. This should never happen, please report a bug.",
                    space_view_blueprint.display_name_or_default()
                );
            }

            let ctx: &'a ViewerContext<'_> = self.ctx;
            let view = space_view_blueprint;
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

            let class = space_view_blueprint.class(self.ctx.space_view_class_registry);
            execute_systems_for_space_view(ctx, view, latest_at, self.view_states.get_mut_or_create(*view_id, class))
        });

        let class = space_view_blueprint.class(self.ctx.space_view_class_registry);
        let view_state = self.view_states.get_mut_or_create(*view_id, class);

        ui.scope(|ui| {
            class
                .ui(self.ctx, ui, view_state, &query, system_output)
                .unwrap_or_else(|err| {
                    re_log::error!(
                        "Error in space view UI (class: {}, display name: {}): {err}",
                        space_view_blueprint.class_identifier(),
                        class.display_name(),
                    );
                });
        });

        Default::default()
    }

    fn tab_title_for_pane(&mut self, space_view_id: &SpaceViewId) -> egui::WidgetText {
        if let Some(space_view) = self.viewport_blueprint.view(space_view_id) {
            // Note: the formatting for unnamed space views is handled by `TabWidget::new()`
            space_view.display_name_or_default().as_ref().into()
        } else {
            // All panes are space views, so this shouldn't happen unless we have a bug
            re_log::warn_once!("SpaceViewId missing during egui_tiles");
            self.ctx.egui_ctx.error_text("Internal error").into()
        }
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn tab_ui(
        &mut self,
        tiles: &mut egui_tiles::Tiles<SpaceViewId>,
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
            egui_tiles::Tile::Pane(space_view_id) => Some(Item::SpaceView(*space_view_id)),

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
            self.ctx.select_hovered_on_click(&response, item);
        }

        response
    }

    fn drag_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
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

    fn retain_pane(&mut self, space_view_id: &SpaceViewId) -> bool {
        self.viewport_blueprint
            .space_views
            .contains_key(space_view_id)
    }

    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let Some(active) = tabs.active.and_then(|active| tiles.get(active)) else {
            return;
        };
        let egui_tiles::Tile::Pane(space_view_id) = active else {
            return;
        };
        let space_view_id = *space_view_id;

        let Some(space_view_blueprint) = self.viewport_blueprint.view(&space_view_id) else {
            return;
        };
        let num_space_views = tiles.tiles().filter(|tile| tile.is_pane()).count();

        ui.add_space(8.0); // margin within the frame

        if *self.maximized == Some(space_view_id) {
            // Show minimize-button:
            if ui
                .small_icon_button(&re_ui::icons::MINIMIZE)
                .on_hover_text("Restore - show all spaces")
                .clicked()
            {
                *self.maximized = None;
            }
        } else if num_space_views > 1 {
            // Show maximize-button:
            if ui
                .small_icon_button(&re_ui::icons::MAXIMIZE)
                .on_hover_text("Maximize space view")
                .clicked()
            {
                *self.maximized = Some(space_view_id);
                // Just maximize - don't select. See https://github.com/rerun-io/rerun/issues/2861
            }
        }

        let space_view_class = space_view_blueprint.class(self.ctx.space_view_class_registry);

        // give the view a chance to display some extra UI in the top bar.
        let view_state = self
            .view_states
            .get_mut_or_create(space_view_id, space_view_class);
        space_view_class
            .extra_title_bar_ui(
                self.ctx,
                ui,
                view_state,
                &space_view_blueprint.space_origin,
                space_view_id,
            )
            .unwrap_or_else(|err| {
                re_log::error!(
                    "Error in view title bar UI (class: {}, display name: {}): {err}",
                    space_view_blueprint.class_identifier(),
                    space_view_class.display_name(),
                );
            });

        let help_markdown = space_view_class.help_markdown(self.ctx.egui_ctx);
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
        match edit_action {
            EditAction::TileDropped => {
                // TODO(ab): when we finally stop using egui_tiles as application-level data
                //                  structure, this work-around should be unnecessary.

                // The continuous simplification options are considerably reduced when the additive
                // workflow is enabled. Due to the egui_tiles -> blueprint synchronisation process,
                // drag and drop operation often lead to many spurious empty containers. To work
                // around this, we run a simplification pass when a drop occurs.

                self.deferred_commands
                    .lock()
                    .push(ViewportCommand::SimplifyContainer(
                        self.root_container_id,
                        egui_tiles::SimplificationOptions {
                            prune_empty_tabs: true,
                            prune_empty_containers: false,
                            prune_single_child_tabs: true,
                            prune_single_child_containers: false,
                            all_panes_must_have_tabs: true,
                            join_nested_linear_containers: false,
                        },
                    ));

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
/// which is either a Pane with a Space View, or a container,
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
        tiles: &'a egui_tiles::Tiles<SpaceViewId>,
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
            Some(egui_tiles::Tile::Pane(space_view_id)) => {
                if let Some(space_view) = tab_viewer.viewport_blueprint.view(space_view_id) {
                    TabDesc {
                        label: tab_viewer.tab_title_for_pane(space_view_id),
                        user_named: space_view.display_name.is_some(),
                        icon: space_view
                            .class(tab_viewer.ctx.space_view_class_registry)
                            .icon(),
                        item: Some(Item::SpaceView(*space_view_id)),
                    }
                } else {
                    re_log::warn_once!("Space view {space_view_id} not found");

                    TabDesc {
                        label: tab_viewer
                            .ctx
                            .egui_ctx
                            .error_text("Unknown space view")
                            .into(),
                        icon: &re_ui::icons::SPACE_VIEW_GENERIC,
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
                        icon: &re_ui::icons::SPACE_VIEW_GENERIC,
                        user_named: false,
                        item: None,
                    }
                }
            }
            None => {
                re_log::warn_once!("Tile {tile_id:?} not found");

                TabDesc {
                    label: tab_viewer.ctx.egui_ctx.error_text("Internal error").into(),
                    icon: &re_ui::icons::SPACE_VIEW_UNKNOWN,
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
