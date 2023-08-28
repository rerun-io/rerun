//! The viewport panel.
//!
//! Contains all space views.

use std::collections::BTreeMap;

use ahash::HashMap;

use re_viewer_context::{
    CommandSender, Item, SpaceViewClassName, SpaceViewClassRegistry, SpaceViewHighlights,
    SpaceViewId, SpaceViewState, ViewerContext,
};

use crate::{
    space_view_entity_picker::SpaceViewEntityPicker,
    space_view_heuristics::default_created_space_views,
    space_view_highlights::highlights_for_space_view, viewport_blueprint::load_viewport_blueprint,
    SpaceInfoCollection, SpaceViewBlueprint, ViewportBlueprint,
};

// ----------------------------------------------------------------------------
/// State for the [`Viewport`] that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewportState {
    pub(crate) space_view_entity_window: Option<SpaceViewEntityPicker>,
    space_view_states: HashMap<SpaceViewId, Box<dyn SpaceViewState>>,
}

impl ViewportState {
    pub fn space_view_state_mut(
        &mut self,
        space_view_class_registry: &SpaceViewClassRegistry,
        space_view_id: SpaceViewId,
        space_view_class: &SpaceViewClassName,
    ) -> &mut dyn SpaceViewState {
        self.space_view_states
            .entry(space_view_id)
            .or_insert_with(|| {
                space_view_class_registry
                    .get_class_or_log_error(space_view_class)
                    .new_state()
            })
            .as_mut()
    }
}

// ----------------------------------------------------------------------------

/// Defines the layout of the Viewport
pub struct Viewport<'a, 'b> {
    pub blueprint: ViewportBlueprint<'a>,
    snapshot: ViewportBlueprint<'a>,

    pub state: &'b mut ViewportState,
}

impl<'a, 'b> Viewport<'a, 'b> {
    pub fn from_db(blueprint_db: &'a re_data_store::StoreDb, state: &'b mut ViewportState) -> Self {
        re_tracing::profile_function!();

        let blueprint = load_viewport_blueprint(blueprint_db);

        let snapshot = blueprint.clone();

        Self {
            snapshot,
            blueprint,
            state,
        }
    }

    pub fn sync_blueprint_changes(&self, command_sender: &CommandSender) {
        self.blueprint
            .sync_viewport_blueprint(&self.snapshot, command_sender);
    }

    pub fn show_add_remove_entities_window(&mut self, space_view_id: SpaceViewId) {
        self.state.space_view_entity_window = Some(SpaceViewEntityPicker { space_view_id });
    }

    pub fn viewport_ui(&mut self, ui: &mut egui::Ui, ctx: &'a mut ViewerContext<'_>) {
        let Viewport {
            blueprint,
            snapshot: _,
            state,
        } = self;

        if let Some(window) = &mut state.space_view_entity_window {
            if let Some(space_view) = blueprint.space_views.get_mut(&window.space_view_id) {
                if !window.ui(ctx, ui, space_view) {
                    state.space_view_entity_window = None;
                }
            } else {
                // The space view no longer exist, close the window!
                state.space_view_entity_window = None;
            }
        }

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return;
        }

        if let Some(space_view_id) = blueprint.maximized {
            if !blueprint.space_views.contains_key(&space_view_id) {
                blueprint.maximized = None; // protect against bad deserialized data
            } else if let Some(tile_id) = blueprint.tree.tiles.find_pane(&space_view_id) {
                if !blueprint.tree.tiles.is_visible(tile_id) {
                    blueprint.maximized = None; // Automatically de-maximize views that aren't visible anymore.
                }
            }
        }

        let mut maximized_tree;

        let tree = if let Some(space_view_id) = blueprint.maximized {
            let mut tiles = egui_tiles::Tiles::default();
            let root = tiles.insert_pane(space_view_id);
            maximized_tree = egui_tiles::Tree::new(root, tiles);
            &mut maximized_tree
        } else {
            if blueprint.tree.is_empty() {
                blueprint.tree = super::auto_layout::tree_from_space_views(
                    ctx.space_view_class_registry,
                    &blueprint.space_views,
                );
            }
            &mut blueprint.tree
        };

        let mut tab_viewer = TabViewer {
            viewport_state: state,
            ctx,
            space_views: &mut blueprint.space_views,
            maximized: &mut blueprint.maximized,
        };

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = re_ui::ReUi::view_padding();

            re_tracing::profile_scope!("tree.ui");
            tree.ui(&mut tab_viewer, ui);
        });
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        re_tracing::profile_function!();

        for space_view in self.blueprint.space_views.values_mut() {
            let space_view_state = self.state.space_view_state_mut(
                ctx.space_view_class_registry,
                space_view.id,
                space_view.class_name(),
            );
            space_view.on_frame_start(ctx, spaces_info, space_view_state);
        }

        if self.blueprint.auto_space_views {
            for space_view_candidate in default_created_space_views(ctx, spaces_info) {
                if self.should_auto_add_space_view(&space_view_candidate) {
                    self.blueprint.add_space_view(space_view_candidate);
                }
            }
        }
    }

    fn should_auto_add_space_view(&self, space_view_candidate: &SpaceViewBlueprint) -> bool {
        for existing_view in self.blueprint.space_views.values() {
            if existing_view.space_origin == space_view_candidate.space_origin {
                if existing_view.entities_determined_by_user {
                    // Since the user edited a space view with the same space path, we can't be sure our new one isn't redundant.
                    // So let's skip that.
                    return false;
                }

                if space_view_candidate
                    .data_blueprint
                    .entity_paths()
                    .is_subset(existing_view.data_blueprint.entity_paths())
                {
                    // This space view wouldn't add anything we haven't already
                    return false;
                }
            }
        }

        true
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    #[inline]
    pub fn is_item_valid(&self, item: &Item) -> bool {
        self.blueprint.is_item_valid(item)
    }
}

// ----------------------------------------------------------------------------

struct TabViewer<'a, 'b> {
    viewport_state: &'a mut ViewportState,
    ctx: &'a mut ViewerContext<'b>,
    space_views: &'a mut BTreeMap<SpaceViewId, SpaceViewBlueprint>,
    maximized: &'a mut Option<SpaceViewId>,
}

impl<'a, 'b> egui_tiles::Behavior<SpaceViewId> for TabViewer<'a, 'b> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        space_view_id: &mut SpaceViewId,
    ) -> egui_tiles::UiResponse {
        re_tracing::profile_function!();

        let highlights =
            highlights_for_space_view(self.ctx.selection_state(), *space_view_id, self.space_views);
        let space_view_blueprint = self
            .space_views
            .get_mut(space_view_id)
            .expect("Should have been populated beforehand");
        let space_view_state = self.viewport_state.space_view_state_mut(
            self.ctx.space_view_class_registry,
            space_view_blueprint.id,
            space_view_blueprint.class_name(),
        );

        space_view_ui(
            self.ctx,
            ui,
            space_view_blueprint,
            space_view_state,
            &highlights,
        );

        Default::default()
    }

    fn tab_title_for_pane(&mut self, space_view_id: &SpaceViewId) -> egui::WidgetText {
        let Some(space_view) = self.space_views.get_mut(space_view_id) else {
            // this shouldn't happen unless we have a bug
            re_log::debug_once!("SpaceViewId missing during egui_tiles");
            return "internal_error".into();
        };

        let mut text =
            egui::WidgetText::RichText(egui::RichText::new(space_view.display_name.clone()));

        if self
            .ctx
            .selection()
            .contains(&Item::SpaceView(*space_view_id))
        {
            // Show that it is selected:
            let egui_ctx = &self.ctx.re_ui.egui_ctx;
            let selection_bg_color = egui_ctx.style().visuals.selection.bg_fill;
            text = text.background_color(selection_bg_color);
        }

        text
    }

    fn on_tab_button(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        tile_id: egui_tiles::TileId,
        button_response: &egui::Response,
    ) {
        if button_response.clicked() {
            if let Some(egui_tiles::Tile::Pane(space_view_id)) = tiles.get(tile_id) {
                self.ctx
                    .set_single_selection(&Item::SpaceView(*space_view_id));
            }
        }
    }

    fn retain_pane(&mut self, space_view_id: &SpaceViewId) -> bool {
        self.space_views.contains_key(space_view_id)
    }

    fn top_bar_rtl_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        tabs: &egui_tiles::Tabs,
    ) {
        let Some(active) = tabs.active.and_then(|active| tiles.get(active)) else { return; };
        let egui_tiles::Tile::Pane(space_view_id) = active else { return; };
        let space_view_id = *space_view_id;

        let Some(space_view) = self.space_views.get(&space_view_id) else { return; };
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
                self.ctx
                    .set_single_selection(&Item::SpaceView(space_view_id));
            }
        }

        let help_text = space_view
            .class(self.ctx.space_view_class_registry)
            .help_text(self.ctx.re_ui);
        re_ui::help_hover_button(ui).on_hover_text(help_text);
    }

    // Styling:

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tile_id: egui_tiles::TileId,
        _active: bool,
    ) -> egui::Stroke {
        egui::Stroke::NONE
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        re_ui::ReUi::title_bar_height()
    }

    /// What are the rules for simplifying the tree?
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

fn space_view_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_blueprint: &mut SpaceViewBlueprint,
    space_view_state: &mut dyn SpaceViewState,
    space_view_highlights: &SpaceViewHighlights,
) {
    let Some(latest_at) = ctx.rec_cfg.time_ctrl.time_int() else {
        ui.centered_and_justified(|ui| {
            ui.weak("No time selected");
        });
        return
    };

    space_view_blueprint.scene_ui(space_view_state, ctx, ui, latest_at, space_view_highlights);
}
