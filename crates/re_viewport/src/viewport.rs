//! The viewport panel.
//!
//! Contains all space views.

use std::collections::BTreeMap;

use ahash::HashMap;

use egui_tiles::Behavior as _;
use once_cell::sync::Lazy;
use re_data_store::EntityPropertyMap;
use re_data_ui::item_ui;

use re_ui::{Icon, ReUi};
use re_viewer_context::{
    Item, SpaceViewClassIdentifier, SpaceViewClassRegistry, SpaceViewId, SpaceViewState,
    SystemExecutionOutput, ViewQuery, ViewerContext,
};

use crate::{
    space_view_entity_picker::SpaceViewEntityPicker,
    space_view_heuristics::default_created_space_views,
    space_view_highlights::highlights_for_space_view,
    system_execution::execute_systems_for_space_views, SpaceInfoCollection, SpaceViewBlueprint,
    ViewportBlueprint,
};

// State for each `SpaceView` including both the auto properties and
// the internal state of the space view itself.
pub struct PerSpaceViewState {
    pub auto_properties: EntityPropertyMap,
    pub space_view_state: Box<dyn SpaceViewState>,
}

// ----------------------------------------------------------------------------
/// State for the [`Viewport`] that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewportState {
    space_view_entity_window: SpaceViewEntityPicker,
    space_view_states: HashMap<SpaceViewId, PerSpaceViewState>,

    /// List of all space views that were visible *on screen* (excluding e.g. unselected tabs) the last frame.
    ///
    /// TODO(rerun-io/egui_tiles#34): This is needed because we don't know which space views will be visible until we have drawn them.
    space_views_displayed_last_frame: Vec<SpaceViewId>,
}

static DEFAULT_PROPS: Lazy<EntityPropertyMap> = Lazy::<EntityPropertyMap>::new(Default::default);

impl ViewportState {
    pub fn space_view_state_mut(
        &mut self,
        space_view_class_registry: &SpaceViewClassRegistry,
        space_view_id: SpaceViewId,
        space_view_class: &SpaceViewClassIdentifier,
    ) -> &mut PerSpaceViewState {
        self.space_view_states
            .entry(space_view_id)
            .or_insert_with(|| PerSpaceViewState {
                auto_properties: Default::default(),
                space_view_state: space_view_class_registry
                    .get_class_or_log_error(space_view_class)
                    .new_state(),
            })
    }

    pub fn space_view_props(&self, space_view_id: SpaceViewId) -> &EntityPropertyMap {
        self.space_view_states
            .get(&space_view_id)
            .map_or(&DEFAULT_PROPS, |state| &state.auto_properties)
    }
}

/// Mutation actions to perform on the tree at the end of the frame. These messages are sent by the mutation APIs from
/// [`crate::ViewportBlueprint`].
#[derive(Clone)]
pub enum TreeAction {
    /// Add a new space view to the provided container (or the root if `None`).
    AddSpaceView(SpaceViewId, Option<egui_tiles::TileId>),

    /// Add a new container of the provided kind to the provided container (or the root if `None`).
    AddContainer(egui_tiles::ContainerKind, Option<egui_tiles::TileId>),

    /// Change the kind of a container.
    SetContainerKind(egui_tiles::TileId, egui_tiles::ContainerKind),

    /// Ensure the tab for the provided space view is focused (see [`egui_tiles::Tree::make_active`]).
    FocusTab(SpaceViewId),

    /// Remove a tile and all its children.
    Remove(egui_tiles::TileId),

    /// Simplify the specified subtree with the provided options
    SimplifyTree(egui_tiles::TileId, egui_tiles::SimplificationOptions),
}

// ----------------------------------------------------------------------------

/// Defines the layout of the Viewport
pub struct Viewport<'a, 'b> {
    /// The blueprint that drives this viewport. This is the source of truth from the store
    /// for this frame.
    pub blueprint: &'a ViewportBlueprint,

    /// The persistent state of the viewport that is not saved to the store but otherwise
    /// persis frame-to-frame.
    pub state: &'b mut ViewportState,

    /// The [`egui_tiles::Tree`] tree that actually manages blueprint layout. This tree needs
    /// to be mutable for things like drag-and-drop and is ultimately saved back to the store.
    /// at the end of the frame if edited.
    pub tree: egui_tiles::Tree<SpaceViewId>,
    pub edited: bool,

    /// Actions to perform at the end of the frame.
    ///
    /// We delay any modifications to the tree until the end of the frame,
    /// so that we don't mutate something while inspecting it.
    tree_action_receiver: std::sync::mpsc::Receiver<TreeAction>,
}

impl<'a, 'b> Viewport<'a, 'b> {
    pub fn new(
        blueprint: &'a ViewportBlueprint,
        state: &'b mut ViewportState,
        space_view_class_registry: &SpaceViewClassRegistry,
        tree_action_receiver: std::sync::mpsc::Receiver<TreeAction>,
    ) -> Self {
        re_tracing::profile_function!();

        let mut edited = false;

        // If the blueprint tree is empty/missing we need to auto-layout.
        let tree = if blueprint.tree.is_empty() && !blueprint.space_views.is_empty() {
            edited = true;
            super::auto_layout::tree_from_space_views(
                space_view_class_registry,
                &blueprint.space_views,
            )
        } else {
            blueprint.tree.clone()
        };

        Self {
            blueprint,
            state,
            tree,
            edited,
            tree_action_receiver,
        }
    }

    pub fn show_add_remove_entities_window(&mut self, space_view_id: SpaceViewId) {
        self.state.space_view_entity_window.open(space_view_id);
    }

    pub fn viewport_ui(&mut self, ui: &mut egui::Ui, ctx: &'a ViewerContext<'_>) {
        self.state
            .space_view_entity_window
            .ui(ui, ctx, self.blueprint);

        let Viewport {
            blueprint, state, ..
        } = self;

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
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

        let mut maximized_tree;

        let tree = if let Some(space_view_id) = blueprint.maximized {
            let mut tiles = egui_tiles::Tiles::default();
            let root = tiles.insert_pane(space_view_id);
            maximized_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);
            &mut maximized_tree
        } else {
            &mut self.tree
        };

        let executed_systems_per_space_view = execute_systems_for_space_views(
            ctx,
            std::mem::take(&mut state.space_views_displayed_last_frame),
            &blueprint.space_views,
        );

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = re_ui::ReUi::view_padding();

            re_tracing::profile_scope!("tree.ui");

            let mut tab_viewer = TabViewer {
                viewport_state: state,
                ctx,
                space_views: &blueprint.space_views,
                maximized: &mut maximized,
                edited: false,
                space_views_displayed_current_frame: Vec::new(),
                executed_systems_per_space_view,
            };

            tree.ui(&mut tab_viewer, ui);

            // Detect if the user has moved a tab or similar.
            // If so we can no longer automatically change the layout without discarding user edits.
            let is_dragging_a_tile = tree.dragged_id(ui.ctx()).is_some();
            if tab_viewer.edited || is_dragging_a_tile {
                if blueprint.auto_layout {
                    re_log::trace!(
                        "The user is manipulating the egui_tiles tree - will no longer auto-layout"
                    );
                }

                blueprint.set_auto_layout(false, ctx);
            }

            self.edited |= tab_viewer.edited;

            state.space_views_displayed_last_frame = tab_viewer.space_views_displayed_current_frame;
        });

        self.blueprint.set_maximized(maximized, ctx);
    }

    pub fn on_frame_start(&mut self, ctx: &ViewerContext<'_>, spaces_info: &SpaceInfoCollection) {
        re_tracing::profile_function!();

        for space_view in self.blueprint.space_views.values() {
            let PerSpaceViewState {
                auto_properties,
                space_view_state,
            } = self.state.space_view_state_mut(
                ctx.space_view_class_registry,
                space_view.id,
                space_view.class_identifier(),
            );

            space_view.on_frame_start(ctx, space_view_state.as_mut(), auto_properties);
        }

        if self.blueprint.auto_space_views {
            let mut new_space_views = vec![];
            for space_view_candidate in
                default_created_space_views(ctx, spaces_info, ctx.entities_per_system_per_class)
            {
                if self.should_auto_add_space_view(&new_space_views, &space_view_candidate) {
                    new_space_views.push(space_view_candidate);
                }
            }

            self.blueprint
                .add_space_views(new_space_views.into_iter(), ctx, None);
        }
    }

    /// Simplify the tile tree.
    ///
    /// If a `tile_id` is provided, only that subtree will be simplified.
    pub fn simplify_tree(
        &mut self,
        tile_id: Option<egui_tiles::TileId>,
        simplification_options: egui_tiles::SimplificationOptions,
    ) {
        if let Some(tile_id) = tile_id.or(self.tree.root) {
            self.deferred_tree_actions
                .simplify
                .push((tile_id, simplification_options));
        }
    }

    fn should_auto_add_space_view(
        &self,
        already_added: &[SpaceViewBlueprint],
        space_view_candidate: &SpaceViewBlueprint,
    ) -> bool {
        re_tracing::profile_function!();

        for existing_view in self
            .blueprint
            .space_views
            .values()
            .chain(already_added.iter())
        {
            if existing_view.space_origin == space_view_candidate.space_origin {
                if existing_view.entities_determined_by_user {
                    // Since the user edited a space view with the same space path, we can't be sure our new one isn't redundant.
                    // So let's skip that.
                    return false;
                }
                if existing_view
                    .queries
                    .iter()
                    .zip(space_view_candidate.queries.iter())
                    .all(|(q1, q2)| q1.is_equivalent(q2))
                {
                    // This space view wouldn't add anything we haven't already
                    return false;
                }
            }
        }

        true
    }

    /// Process any deferred `TreeActions` and then sync to blueprint
    pub fn update_and_sync_tile_tree_to_blueprint(mut self, ctx: &ViewerContext<'_>) {
        // At the end of the Tree-UI, we can safely apply deferred actions.

        let mut reset = false;

        for tree_action in self.tree_action_receiver.try_iter() {
            match tree_action {
                TreeAction::AddSpaceView(space_view_id, parent_container) => {
                    if self.blueprint.auto_layout {
                        // Re-run the auto-layout next frame:
                        re_log::trace!(
                            "Added a space view with no user edits yet - will re-run auto-layout"
                        );

                        reset = true;
                    } else if let Some(parent_id) = parent_container.or(self.tree.root) {
                        let tile_id = self.tree.tiles.insert_pane(space_view_id);
                        if let Some(egui_tiles::Tile::Container(container)) =
                            self.tree.tiles.get_mut(parent_id)
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

                    self.edited = true;
                }
                TreeAction::AddContainer(container_kind, parent_container) => {
                    if let Some(parent_id) = parent_container.or(self.tree.root) {
                        let tile_id = self
                            .tree
                            .tiles
                            .insert_container(egui_tiles::Container::new(container_kind, vec![]));
                        if let Some(egui_tiles::Tile::Container(container)) =
                            self.tree.tiles.get_mut(parent_id)
                        {
                            re_log::trace!("Inserting new space view into container {parent_id:?}");
                            container.add_child(tile_id);
                        } else {
                            re_log::trace!(
                                "Parent or root was not a container - will re-run auto-layout"
                            );
                            reset = true;
                        }
                    } else {
                        re_log::trace!("No root found - will re-run auto-layout");
                    }

                    self.edited = true;
                }
                TreeAction::SetContainerKind(container_id, container_kind) => {
                    if let Some(egui_tiles::Tile::Container(container)) =
                        self.tree.tiles.get_mut(container_id)
                    {
                        re_log::trace!("Mutating container {container_id:?} to {container_kind:?}");
                        container.set_kind(container_kind);
                    } else {
                        re_log::trace!("No root found - will re-run auto-layout");
                    }

                    self.edited = true;
                }
                TreeAction::FocusTab(space_view_id) => {
                    let found = self.tree.make_active(|_, tile| match tile {
                        egui_tiles::Tile::Pane(this_space_view_id) => {
                            *this_space_view_id == space_view_id
                        }
                        egui_tiles::Tile::Container(_) => false,
                    });
                    re_log::trace!(
                        "Found tab to focus on for space view ID {space_view_id}: {found}"
                    );
                    self.edited = true;
                }
                TreeAction::Remove(tile_id) => {
                    for tile in self.tree.tiles.remove_recursively(tile_id) {
                        re_log::trace!("Removing tile {tile_id:?}");
                        if let egui_tiles::Tile::Pane(space_view_id) = tile {
                            re_log::trace!("Removing space view {space_view_id}");
                            self.tree.tiles.remove(tile_id);
                            self.blueprint.remove_space_view(&space_view_id, ctx);
                        }
                    }

                    if Some(tile_id) == self.tree.root {
                        self.tree.root = None;
                    }
                    self.edited = true;
                }
                TreeAction::SimplifyTree(tile_id, options) => {
                    re_log::trace!("Simplifying tree with options: {options:?}");
                    self.tree.simplify_tile(tile_id, &options);
                    self.edited = true;
                }
            }
        }

        for (tile_id, simplify_options) in simplify {
            re_log::trace!("Simplifying tile {tile_id:?}");
            self.tree.simplify_tile(tile_id, &simplify_options);
            self.edited = true;
        }

        if reset {
            // We don't run auto-layout here since the new space views also haven't been
            // written to the store yet.
            re_log::trace!("Clearing the blueprint tree to force reset on the next frame");
            self.tree = egui_tiles::Tree::empty("viewport_tree");
            self.edited = true;
        }

        // Finally, save any edits to the blueprint tree
        // This is a no-op if the tree hasn't changed.
        if ctx.app_options.experimental_container_blueprints {
            if self.edited {
                // TODO(abey79): Decide what simplification to do here. Some of this
                // might need to get rolled into the save logic instead.

                if false {
                    // Simplify before we save the tree. Normally additional simplification will
                    // happen on the next render loop, but that's too late -- unsimplified
                    // changes will be baked into the tree.
                    let options = egui_tiles::SimplificationOptions {
                        all_panes_must_have_tabs: true,
                        ..Default::default()
                    };
                    self.tree.simplify(&options);
                }

                self.blueprint.save_tree_as_containers(&self.tree, ctx);
            }
        } else {
            self.blueprint.set_tree(&self.tree, ctx);
        }
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    #[inline]
    pub fn is_item_valid(&self, item: &Item) -> bool {
        self.blueprint.is_item_valid(item)
    }
}

// ----------------------------------------------------------------------------

/// `egui_tiles` has _tiles_ which are either _containers_ or _panes_.
///
/// In our case, each pane is a space view,
/// while containers are just groups of things.
struct TabViewer<'a, 'b> {
    viewport_state: &'a mut ViewportState,
    ctx: &'a ViewerContext<'b>,
    space_views: &'a BTreeMap<SpaceViewId, SpaceViewBlueprint>,
    maximized: &'a mut Option<SpaceViewId>,

    /// List of query & system execution results for each space view.
    executed_systems_per_space_view: HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)>,

    /// List of all space views drawn this frame.
    ///
    /// TODO(rerun-io/egui_tiles#34): It should be possible to predict which space views will be drawn.
    space_views_displayed_current_frame: Vec<SpaceViewId>,

    /// The user edited the tree.
    edited: bool,
}

impl<'a, 'b> egui_tiles::Behavior<SpaceViewId> for TabViewer<'a, 'b> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        space_view_id: &mut SpaceViewId,
    ) -> egui_tiles::UiResponse {
        re_tracing::profile_function!();

        let Some(space_view_blueprint) = self.space_views.get(space_view_id) else {
            return Default::default();
        };

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return Default::default();
        }

        let Some(latest_at) = self.ctx.rec_cfg.time_ctrl.read().time_int() else {
            ui.centered_and_justified(|ui| {
                ui.weak("No time selected");
            });
            return Default::default();
        };

        // TODO(rerun-io/egui_tiles#34): If we haven't executed the system yet ahead of time, we should do so now.
        // This is needed because we merely "guess" which systems we are going to need.
        let (query, system_output) =
            if let Some(result) = self.executed_systems_per_space_view.remove(space_view_id) {
                result
            } else {
                let highlights = highlights_for_space_view(
                    self.ctx.selection_state(),
                    *space_view_id,
                    self.space_views,
                );
                space_view_blueprint.execute_systems(self.ctx, latest_at, highlights)
            };

        let PerSpaceViewState {
            auto_properties: _,
            space_view_state,
        } = self.viewport_state.space_view_state_mut(
            self.ctx.space_view_class_registry,
            space_view_blueprint.id,
            space_view_blueprint.class_identifier(),
        );

        self.space_views_displayed_current_frame
            .push(space_view_blueprint.id);

        space_view_blueprint.scene_ui(
            space_view_state.as_mut(),
            self.ctx,
            ui,
            &query,
            system_output,
        );

        Default::default()
    }

    fn tab_title_for_pane(&mut self, space_view_id: &SpaceViewId) -> egui::WidgetText {
        if let Some(space_view) = self.space_views.get(space_view_id) {
            space_view.display_name.clone().into()
        } else {
            // All panes are space views, so this shouldn't happen unless we have a bug
            re_log::warn_once!("SpaceViewId missing during egui_tiles");
            self.ctx.re_ui.error_text("Internal error").into()
        }
    }

    fn tab_title_for_tile(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        tile_id: egui_tiles::TileId,
    ) -> egui::WidgetText {
        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                egui_tiles::Tile::Pane(pane) => self.tab_title_for_pane(pane),

                // E.g. a tab with a grid of other tiles
                egui_tiles::Tile::Container(container) => {
                    format!("{:?} Container", container.kind()).into()
                }
            }
        } else {
            re_log::warn_once!("SpaceViewId missing during tab_title_for_tile");
            self.ctx.re_ui.error_text("Internal error").into()
        }
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn tab_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        ui: &mut egui::Ui,
        id: egui::Id,
        tile_id: egui_tiles::TileId,
        active: bool,
        is_being_dragged: bool,
    ) -> egui::Response {
        let tab_widget = TabWidget::new(self, ui, tiles, tile_id, active, 1.0);

        let response = ui.interact(tab_widget.rect, id, egui::Sense::click_and_drag());

        // Show a gap when dragged
        if ui.is_rect_visible(tab_widget.rect) && !is_being_dragged {
            tab_widget.paint(ui);
        }

        if let Some(egui_tiles::Tile::Pane(space_view_id)) = tiles.get(tile_id) {
            item_ui::select_hovered_on_click(self.ctx, &response, Item::SpaceView(*space_view_id));
        }

        response
    }

    fn drag_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
    ) {
        let tab_widget = TabWidget::new(self, ui, tiles, tile_id, true, 0.5);

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
        self.space_views.contains_key(space_view_id)
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

        let Some(space_view) = self.space_views.get(&space_view_id) else {
            return;
        };
        let num_space_views = tiles.tiles().filter(|tile| tile.is_pane()).count();

        ui.add_space(8.0); // margin within the frame

        if *self.maximized == Some(space_view_id) {
            // Show minimize-button:
            if self
                .ctx
                .re_ui
                .small_icon_button(ui, &re_ui::icons::MINIMIZE)
                .on_hover_text("Restore - show all spaces")
                .clicked()
            {
                *self.maximized = None;
            }
        } else if num_space_views > 1 {
            // Show maximize-button:
            if self
                .ctx
                .re_ui
                .small_icon_button(ui, &re_ui::icons::MAXIMIZE)
                .on_hover_text("Maximize Space View")
                .clicked()
            {
                *self.maximized = Some(space_view_id);
                // Just maximize - don't select. See https://github.com/rerun-io/rerun/issues/2861
            }
        }

        let help_text = space_view
            .class(self.ctx.space_view_class_registry)
            .help_text(self.ctx.re_ui);
        re_ui::help_hover_button(ui).on_hover_text(help_text);
    }

    // Styling:

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        self.ctx.re_ui.design_tokens.tab_bar_color
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
        re_ui::ReUi::title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    ///
    /// These options are applied on every frame by `egui_tiles`.
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            prune_empty_tabs: false,
            all_panes_must_have_tabs: true,
            prune_empty_containers: false,
            prune_single_child_tabs: false,
            prune_single_child_containers: false,
            join_nested_linear_containers: true,
        }
    }

    // Callbacks:

    fn on_edit(&mut self) {
        self.edited = true;
    }
}

/// A tab button for a tab in the viewport.
///
/// The tab can contain any `egui_tiles::Tile`,
/// which is either a Pane with a Space View, or a Container,
/// e.g. a grid of tiles.
struct TabWidget {
    galley: egui::widget_text::WidgetTextGalley,
    rect: egui::Rect,
    galley_rect: egui::Rect,
    icon: &'static Icon,
    icon_size: egui::Vec2,
    icon_rect: egui::Rect,
    bg_color: egui::Color32,
    text_color: egui::Color32,
}

impl TabWidget {
    fn new<'a>(
        tab_viewer: &'a mut TabViewer<'_, '_>,
        ui: &'a mut egui::Ui,
        tiles: &'a egui_tiles::Tiles<SpaceViewId>,
        tile_id: egui_tiles::TileId,
        active: bool,
        gamma: f32,
    ) -> Self {
        // Not all tabs are for tiles (space views) - some are for containers (e.g. a grid of space views).
        let space_view = if let Some(egui_tiles::Tile::Pane(space_view_id)) = tiles.get(tile_id) {
            tab_viewer.space_views.get(space_view_id)
        } else {
            None
        };
        let selected = space_view.map_or(false, |space_view| {
            tab_viewer
                .ctx
                .selection()
                .contains_item(&Item::SpaceView(space_view.id))
        });

        let hovered = space_view.map_or(false, |space_view| {
            tab_viewer
                .ctx
                .hovered()
                .contains_item(&Item::SpaceView(space_view.id))
        });

        // tab icon
        let icon_size = ReUi::small_icon_size();
        let icon_width_plus_padding = icon_size.x + ReUi::text_to_icon_padding();
        let icon = space_view.map_or(&re_ui::icons::CONTAINER, |space_view| {
            space_view
                .class(tab_viewer.ctx.space_view_class_registry)
                .icon()
        });

        // tab title
        let text = tab_viewer.tab_title_for_tile(tiles, tile_id);
        let font_id = egui::TextStyle::Button.resolve(ui.style());
        let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);

        let x_margin = tab_viewer.tab_title_spacing(ui.visuals());
        let (_, rect) = ui.allocate_space(egui::vec2(
            galley.size().x + 2.0 * x_margin + icon_width_plus_padding,
            ReUi::title_bar_height(),
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
            .tab_text_color(ui.visuals(), tiles, tile_id, active)
            .gamma_multiply(gamma);

        Self {
            galley,
            rect,
            galley_rect,
            icon,
            icon_size,
            icon_rect,
            bg_color,
            text_color,
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

        ui.painter().galley_with_color(
            egui::Align2::CENTER_CENTER
                .align_size_within_rect(self.galley.size(), self.galley_rect)
                .min,
            self.galley.galley,
            self.text_color,
        );
    }
}
