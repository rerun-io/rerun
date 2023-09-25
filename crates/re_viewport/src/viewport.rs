//! The viewport panel.
//!
//! Contains all space views.

use std::collections::BTreeMap;

use ahash::HashMap;
use egui_tiles::Behavior;
use nohash_hasher::IntMap;

use re_ui::{Icon, ReUi};
use re_viewer_context::{
    CommandSender, Item, SpaceViewClassName, SpaceViewClassRegistry, SpaceViewHighlights,
    SpaceViewId, SpaceViewState, ViewerContext,
};

use crate::{
    space_view_entity_picker::SpaceViewEntityPicker,
    space_view_heuristics::{default_created_space_views, identify_entities_per_system_per_class},
    space_view_highlights::highlights_for_space_view,
    viewport_blueprint::load_viewport_blueprint,
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
    /// The initial state of the Viewport read from the blueprint store on this frame.
    ///
    /// This is used to compare to the possibly mutated blueprint to
    /// determine whether or not we need to save changes back
    /// to the store as part of `sync_blueprint_changes`.
    start_of_frame_snapshot: ViewportBlueprint<'a>,

    // This is what me mutate during the frame.
    pub blueprint: ViewportBlueprint<'a>,

    pub state: &'b mut ViewportState,
}

impl<'a, 'b> Viewport<'a, 'b> {
    pub fn from_db(blueprint_db: &'a re_data_store::StoreDb, state: &'b mut ViewportState) -> Self {
        re_tracing::profile_function!();

        let blueprint = load_viewport_blueprint(blueprint_db);

        let start_of_frame_snapshot = blueprint.clone();

        Self {
            start_of_frame_snapshot,
            blueprint,
            state,
        }
    }

    pub fn sync_blueprint_changes(&self, command_sender: &CommandSender) {
        ViewportBlueprint::sync_viewport_blueprint(
            &self.start_of_frame_snapshot,
            &self.blueprint,
            command_sender,
        );
    }

    pub fn show_add_remove_entities_window(&mut self, space_view_id: SpaceViewId) {
        self.state.space_view_entity_window = Some(SpaceViewEntityPicker { space_view_id });
    }

    pub fn viewport_ui(&mut self, ui: &mut egui::Ui, ctx: &'a mut ViewerContext<'_>) {
        let Viewport {
            blueprint, state, ..
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
            has_been_user_edited: &mut blueprint.has_been_user_edited,
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

        let entities_per_system_per_class = identify_entities_per_system_per_class(ctx);

        for space_view in self.blueprint.space_views.values_mut() {
            let space_view_state = self.state.space_view_state_mut(
                ctx.space_view_class_registry,
                space_view.id,
                space_view.class_name(),
            );

            let empty_map = IntMap::default();
            let entities_per_system_for_class = entities_per_system_per_class
                .get(space_view.class_name())
                .unwrap_or(&empty_map);
            space_view.on_frame_start(
                ctx,
                spaces_info,
                space_view_state,
                entities_per_system_for_class,
            );
        }

        if self.blueprint.auto_space_views {
            for space_view_candidate in
                default_created_space_views(ctx, spaces_info, &entities_per_system_per_class)
            {
                if self.should_auto_add_space_view(&space_view_candidate) {
                    self.blueprint.add_space_view(space_view_candidate);
                }
            }
        }
    }

    fn should_auto_add_space_view(&self, space_view_candidate: &SpaceViewBlueprint) -> bool {
        re_tracing::profile_function!();

        for existing_view in self.blueprint.space_views.values() {
            if existing_view.space_origin == space_view_candidate.space_origin {
                if existing_view.entities_determined_by_user {
                    // Since the user edited a space view with the same space path, we can't be sure our new one isn't redundant.
                    // So let's skip that.
                    return false;
                }
                if existing_view
                    .contents
                    .contains_all_entities_from(&space_view_candidate.contents)
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
    has_been_user_edited: &'a mut bool,
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
        let Some(space_view_blueprint) = self.space_views.get_mut(space_view_id) else {
            return Default::default();
        };

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

        egui::WidgetText::RichText(egui::RichText::new(space_view.display_name.clone()))
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
        let Some(tab_widget) = TabWidget::new(self, ui, tiles, tile_id, active, 1.0) else {
            return egui_tiles::Behavior::<SpaceViewId>::tab_ui(
                self,
                tiles,
                ui,
                id,
                tile_id,
                active,
                is_being_dragged,
            );
        };

        let response = ui.interact(tab_widget.rect, id, egui::Sense::click_and_drag());

        // Show a gap when dragged
        if ui.is_rect_visible(tab_widget.rect) && !is_being_dragged {
            tab_widget.ui(ui);
        }

        self.on_tab_button(tiles, tile_id, &response);

        response
    }

    fn drag_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<SpaceViewId>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
    ) {
        let Some(tab_widget) = TabWidget::new(self, ui, tiles, tile_id, true, 0.5) else {
            return egui_tiles::Behavior::<SpaceViewId>::drag_ui(self, tiles, ui, tile_id);
        };

        let frame = egui::Frame {
            inner_margin: egui::Margin::same(0.),
            outer_margin: egui::Margin::same(0.),
            rounding: egui::Rounding::ZERO,
            shadow: Default::default(),
            fill: egui::Color32::TRANSPARENT,
            stroke: egui::Stroke::NONE,
        };

        frame.show(ui, |ui| {
            tab_widget.ui(ui);
        });
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
                *self.has_been_user_edited = true;
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
                *self.has_been_user_edited = true;
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
        return;
    };

    space_view_blueprint.scene_ui(space_view_state, ctx, ui, latest_at, space_view_highlights);
}

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
    ) -> Option<Self> {
        // make sure we have a space view to work with
        let space_view = if let Some(egui_tiles::Tile::Pane(space_view_id)) = tiles.get(tile_id) {
            tab_viewer.space_views.get(space_view_id)
        } else {
            return None;
        };
        let Some(space_view) = space_view else {
            return None;
        };
        let space_view_id = space_view.id;

        // tab icon
        let icon_size = ReUi::small_icon_size();
        let icon_width_plus_padding = icon_size.x + ReUi::text_to_icon_padding();
        let icon = space_view
            .class(tab_viewer.ctx.space_view_class_registry)
            .icon();

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

        let selected = tab_viewer
            .ctx
            .selection()
            .contains(&Item::SpaceView(space_view_id));
        let bg_color = if selected {
            ui.visuals().selection.bg_fill
        } else {
            tab_viewer.tab_bar_color(ui.visuals())
        };
        let bg_color = bg_color.gamma_multiply(gamma);
        let text_color = tab_viewer
            .tab_text_color(ui.visuals(), tiles, tile_id, active)
            .gamma_multiply(gamma);

        Some(Self {
            galley,
            rect,
            galley_rect,
            icon,
            icon_size,
            icon_rect,
            bg_color,
            text_color,
        })
    }

    fn ui(self, ui: &mut egui::Ui) {
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
