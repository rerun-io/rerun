//! The viewport panel.
//!
//! Contains all space views.

use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_store::EntityPath;
use re_data_ui::item_ui;
use re_space_view::{DataBlueprintGroup, EmptySpaceViewState};
use re_viewer_context::{
    DataBlueprintGroupHandle, Item, SpaceViewClassName, SpaceViewClassRegistry,
    SpaceViewHighlights, SpaceViewId, ViewerContext,
};

use crate::{
    space_info::SpaceInfoCollection,
    space_view::{SpaceViewBlueprint, SpaceViewState},
    space_view_entity_picker::SpaceViewEntityPicker,
    space_view_heuristics::{all_possible_space_views, default_created_space_views},
    space_view_highlights::highlights_for_space_view,
    view_category::ViewCategory,
};

// ----------------------------------------------------------------------------

/// What views are visible?
pub type VisibilitySet = std::collections::BTreeSet<SpaceViewId>;

/// Describes the layout and contents of the Viewport Panel.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Viewport {
    /// Where the space views are stored.
    ///
    /// Not a hashmap in order to preserve the order of the space views.
    pub space_views: BTreeMap<SpaceViewId, SpaceViewBlueprint>,

    /// Which views are visible.
    pub visible: VisibilitySet,

    /// The layouts of all the space views.
    ///
    /// One for each combination of what views are visible.
    /// So if a user toggles the visibility of one SpaceView, we
    /// switch which layout we are using. This is somewhat hacky.
    pub trees: HashMap<VisibilitySet, egui_tiles::Tree<SpaceViewId>>,

    /// Show one tab as maximized?
    pub maximized: Option<SpaceViewId>,

    /// Set to `true` the first time the user messes around with the viewport blueprint.
    pub has_been_user_edited: bool,

    /// Whether or not space views should be created automatically.
    pub auto_space_views: bool,
}

impl Viewport {
    /// Create a default suggested blueprint using some heuristics.
    pub fn new(ctx: &mut ViewerContext<'_>, spaces_info: &SpaceInfoCollection) -> Self {
        re_tracing::profile_function!();

        let mut viewport = Self::default();
        for space_view in default_created_space_views(ctx, spaces_info) {
            viewport.add_space_view(space_view);
        }
        viewport
    }

    pub fn space_view_ids(&self) -> impl Iterator<Item = &SpaceViewId> + '_ {
        self.space_views.keys()
    }

    pub fn space_view(&self, space_view: &SpaceViewId) -> Option<&SpaceViewBlueprint> {
        self.space_views.get(space_view)
    }

    pub fn space_view_mut(
        &mut self,
        space_view_id: &SpaceViewId,
    ) -> Option<&mut SpaceViewBlueprint> {
        self.space_views.get_mut(space_view_id)
    }

    pub(crate) fn remove(&mut self, space_view_id: &SpaceViewId) -> Option<SpaceViewBlueprint> {
        let Self {
            space_views,
            visible,
            trees,
            maximized,
            has_been_user_edited,
            auto_space_views: _,
        } = self;

        *has_been_user_edited = true;

        trees.retain(|vis_set, _| !vis_set.contains(space_view_id));

        if *maximized == Some(*space_view_id) {
            *maximized = None;
        }

        visible.remove(space_view_id);
        space_views.remove(space_view_id)
    }

    /// Show the blueprint panel tree view.
    pub fn tree_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .auto_shrink([true, false])
            .show(ui, |ui| {
                let space_view_ids = self
                    .space_views
                    .keys()
                    .sorted_by_key(|space_view_id| {
                        (
                            &self.space_views[space_view_id].space_origin,
                            *space_view_id,
                        )
                    })
                    .copied()
                    .collect_vec();

                for space_view_id in &space_view_ids {
                    self.space_view_entry_ui(ctx, ui, space_view_id);
                }
            });
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_group(group: &DataBlueprintGroup) -> bool {
        let num_children = group.children.len() + group.entities.len();
        2 <= num_children && num_children <= 3
    }

    fn space_view_entry_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view_id: &SpaceViewId,
    ) {
        let Some(space_view) = self.space_views.get_mut(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        let mut visibility_changed = false;
        let mut removed_space_view = false;
        let mut is_space_view_visible = self.visible.contains(space_view_id);

        let root_group = space_view.data_blueprint.root_group();
        let default_open = Self::default_open_for_group(root_group);
        let collapsing_header_id = ui.id().with(space_view.id);
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            blueprint_row_with_buttons(
                ctx.re_ui,
                ui,
                true,
                is_space_view_visible,
                |ui| {
                    let response = crate::item_ui::space_view_button(ctx, ui, space_view);
                    if response.clicked() {
                        if let Some(tree) = self.trees.get_mut(&self.visible) {
                            focus_tab(tree, space_view_id);
                        }
                    }
                    response
                },
                |re_ui, ui| {
                    visibility_changed =
                        visibility_button_ui(re_ui, ui, true, &mut is_space_view_visible).changed();
                    removed_space_view = re_ui
                        .small_icon_button(ui, &re_ui::icons::REMOVE)
                        .on_hover_text("Remove Space View from the viewport.")
                        .clicked();
                },
            );
        })
        .body(|ui| {
            Self::data_blueprint_tree_ui(
                ctx,
                ui,
                space_view.data_blueprint.root_handle(),
                space_view,
                self.visible.contains(space_view_id),
            );
        });

        if removed_space_view {
            self.remove(space_view_id);
        }

        if visibility_changed {
            self.has_been_user_edited = true;
            if is_space_view_visible {
                self.visible.insert(*space_view_id);
            } else {
                self.visible.remove(space_view_id);
            }
        }
    }

    fn data_blueprint_tree_ui(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        group_handle: DataBlueprintGroupHandle,
        space_view: &mut SpaceViewBlueprint,
        space_view_visible: bool,
    ) {
        let Some(group) = space_view.data_blueprint.group(group_handle) else {
            debug_assert!(false, "Invalid group handle in blueprint group tree");
            return;
        };

        // TODO(andreas): These clones are workarounds against borrowing multiple times from data_blueprint_tree.
        let children = group.children.clone();
        let entities = group.entities.clone();
        let group_name = group.display_name.clone();
        let group_is_visible = group.properties_projected.visible && space_view_visible;

        for entity_path in &entities {
            ui.horizontal(|ui| {
                let mut properties = space_view
                    .data_blueprint
                    .data_blueprints_individual()
                    .get(entity_path);
                blueprint_row_with_buttons(
                    ctx.re_ui,
                    ui,
                    group_is_visible,
                    properties.visible,
                    |ui| {
                        let name = entity_path.iter().last().unwrap().to_string();
                        let label = format!("🔹 {name}");
                        item_ui::data_blueprint_button_to(
                            ctx,
                            ui,
                            label,
                            space_view.id,
                            entity_path,
                        )
                    },
                    |re_ui, ui| {
                        if visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut properties.visible,
                        )
                        .changed()
                        {
                            space_view
                                .data_blueprint
                                .data_blueprints_individual()
                                .set(entity_path.clone(), properties);
                        }
                        if re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove Entity from the space view.")
                            .clicked()
                        {
                            space_view.data_blueprint.remove_entity(entity_path);
                            space_view.entities_determined_by_user = true;
                        }
                    },
                );
            });
        }

        for child_group_handle in &children {
            let Some(child_group) = space_view.data_blueprint.group_mut(*child_group_handle) else {
                debug_assert!(false, "Data blueprint group {group_name} has an invalid child");
                continue;
            };

            let mut remove_group = false;
            let default_open = Self::default_open_for_group(child_group);
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(child_group_handle),
                default_open,
            )
            .show_header(ui, |ui| {
                blueprint_row_with_buttons(
                    ctx.re_ui,
                    ui,
                    group_is_visible,
                    child_group.properties_individual.visible,
                    |ui| {
                        item_ui::data_blueprint_group_button_to(
                            ctx,
                            ui,
                            child_group.display_name.clone(),
                            space_view.id,
                            *child_group_handle,
                        )
                    },
                    |re_ui, ui| {
                        visibility_button_ui(
                            re_ui,
                            ui,
                            group_is_visible,
                            &mut child_group.properties_individual.visible,
                        );
                        if re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove group and all its children from the space view.")
                            .clicked()
                        {
                            remove_group = true;
                        }
                    },
                );
            })
            .body(|ui| {
                Self::data_blueprint_tree_ui(
                    ctx,
                    ui,
                    *child_group_handle,
                    space_view,
                    space_view_visible,
                );
            });
            if remove_group {
                space_view.data_blueprint.remove_group(*child_group_handle);
                space_view.entities_determined_by_user = true;
            }
        }
    }

    pub fn mark_user_interaction(&mut self) {
        self.has_been_user_edited = true;
    }

    pub fn add_space_view(&mut self, mut space_view: SpaceViewBlueprint) -> SpaceViewId {
        let id = space_view.id;

        // Find a unique name for the space view
        let mut candidate_name = space_view.display_name.clone();
        let mut append_count = 1;
        let unique_name = 'outer: loop {
            for view in &self.space_views {
                if candidate_name == view.1.display_name {
                    append_count += 1;
                    candidate_name = format!("{} ({})", space_view.display_name, append_count);

                    continue 'outer;
                }
            }
            break candidate_name;
        };

        space_view.display_name = unique_name;

        self.space_views.insert(id, space_view);
        self.visible.insert(id);
        self.trees.clear(); // Reset them
        id
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        re_tracing::profile_function!();

        for space_view in self.space_views.values_mut() {
            space_view.on_frame_start(ctx, spaces_info);
        }

        if self.auto_space_views {
            for space_view_candidate in default_created_space_views(ctx, spaces_info) {
                if self.should_auto_add_space_view(&space_view_candidate) {
                    self.add_space_view(space_view_candidate);
                }
            }
        }
    }

    fn should_auto_add_space_view(&self, space_view_candidate: &SpaceViewBlueprint) -> bool {
        for existing_view in self.space_views.values() {
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

    pub fn viewport_ui(
        &mut self,
        state: &mut ViewportState,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
    ) {
        if let Some(window) = &mut state.space_view_entity_window {
            if let Some(space_view) = self.space_views.get_mut(&window.space_view_id) {
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

        if let Some(space_view_id) = self.maximized {
            if !self.space_views.contains_key(&space_view_id) {
                self.maximized = None; // protect against bad deserialized data
            }
        }

        let visible_space_views = if let Some(space_view_id) = self.maximized {
            std::iter::once(space_view_id).collect()
        } else {
            self.visible.clone()
        };

        // Lazily create a layout tree based on which SpaceViews should be visible:
        let tree = self
            .trees
            .entry(visible_space_views.clone())
            .or_insert_with(|| {
                super::auto_layout::tree_from_space_views(
                    ctx,
                    ui.available_size(),
                    &visible_space_views,
                    &self.space_views,
                    &state.space_view_states,
                )
            });

        ui.scope(|ui| {
            let mut tab_viewer = TabViewer {
                ctx,
                viewport_state: state,
                space_views: &mut self.space_views,
                maximized: &mut self.maximized,
            };
            ui.spacing_mut().item_spacing.x = re_ui::ReUi::view_padding();
            tree.ui(&mut tab_viewer, ui);
        });
    }

    pub fn add_new_spaceview_button_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        #![allow(clippy::collapsible_if)]

        let icon_image = ctx.re_ui.icon_image(&re_ui::icons::ADD);
        let texture_id = icon_image.texture_id(ui.ctx());
        ui.menu_image_button(texture_id, re_ui::ReUi::small_icon_size(), |ui| {
            ui.style_mut().wrap = Some(false);

            for space_view in all_possible_space_views(ctx, spaces_info)
                .into_iter()
                .sorted_by_key(|space_view| space_view.space_origin.to_string())
            {
                let icon = if let Ok(class) = ctx.space_view_class_registry.get(space_view.class) {
                    class.icon()
                } else {
                    // TODO(andreas): Error handling if class is not found once categories are gone.
                    space_view.category.icon()
                };

                if ctx
                    .re_ui
                    .selectable_label_with_icon(
                        ui,
                        icon,
                        if space_view.space_origin.is_root() {
                            space_view.display_name.clone()
                        } else {
                            space_view.space_origin.to_string()
                        },
                        false,
                    )
                    .clicked()
                {
                    ui.close_menu();
                    let new_space_view_id = self.add_space_view(space_view);
                    ctx.set_single_selection(Item::SpaceView(new_space_view_id));
                }
            }
        })
        .response
        .on_hover_text("Add new space view.");
    }

    pub fn space_views_containing_entity_path(&self, path: &EntityPath) -> Vec<SpaceViewId> {
        self.space_views
            .iter()
            .filter_map(|(space_view_id, space_view)| {
                if space_view.data_blueprint.contains_entity(path) {
                    Some(*space_view_id)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// State for the blueprint that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewportState {
    pub(crate) space_view_entity_window: Option<SpaceViewEntityPicker>,
    space_view_states: HashMap<SpaceViewId, SpaceViewState>,
}

impl ViewportState {
    pub fn show_add_remove_entities_window(&mut self, space_view_id: SpaceViewId) {
        self.space_view_entity_window = Some(SpaceViewEntityPicker { space_view_id });
    }

    pub fn space_view_state(
        &mut self,
        space_view_class_registry: &SpaceViewClassRegistry,
        space_view_id: SpaceViewId,
        space_view_class: SpaceViewClassName,
    ) -> &mut SpaceViewState {
        self.space_view_states
            .entry(space_view_id)
            .or_insert_with(|| {
                let state = if let Ok(state) = space_view_class_registry.get(space_view_class) {
                    state.new_state()
                } else {
                    // TODO(andreas): Enable this once categories are gone.
                    // re_log::error_once!(
                    //     "Space View class \"{}\" is not registered.",
                    //     space_view_class
                    // );
                    Box::<EmptySpaceViewState>::default()
                };
                SpaceViewState {
                    state,
                    selected_tensor: Default::default(),
                    state_time_series: Default::default(),
                    state_bar_chart: Default::default(),
                    state_tensors: Default::default(),
                }
            })
    }
}

/// Show a single button (`add_content`), justified,
/// and show a visibility button if the row is hovered.
///
/// Returns true if visibility changed.
fn blueprint_row_with_buttons(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    enabled: bool,
    visible: bool,
    add_content: impl FnOnce(&mut egui::Ui) -> egui::Response,
    add_on_hover_buttons: impl FnOnce(&re_ui::ReUi, &mut egui::Ui),
) {
    let where_to_add_hover_rect = ui.painter().add(egui::Shape::Noop);

    // Make the main button span the whole width to make it easier to click:
    let main_button_response = ui
        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.style_mut().wrap = Some(false);

            // Turn off the background color of hovered buttons.
            // Why? Because we add a manual hover-effect later.
            // Why? Because we want that hover-effect even when only the visibility button is hovered.
            let visuals = ui.visuals_mut();
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.open.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.open.bg_fill = egui::Color32::TRANSPARENT;

            if ui
                .interact(ui.max_rect(), ui.id(), egui::Sense::hover())
                .hovered()
            {
                // Clip the main button so that the on-hover buttons have room to cover it.
                // Ideally we would only clip the button _text_, not the button background, but that's not possible.
                let mut clip_rect = ui.max_rect();
                let on_hover_buttons_width = 36.0;
                clip_rect.max.x -= on_hover_buttons_width;
                ui.set_clip_rect(clip_rect);
            }

            if !visible || !enabled {
                // Dim the appearance of things added by `add_content`:
                let widget_visuals = &mut ui.visuals_mut().widgets;

                fn dim_color(color: &mut egui::Color32) {
                    *color = color.gamma_multiply(0.5);
                }
                dim_color(&mut widget_visuals.noninteractive.fg_stroke.color);
                dim_color(&mut widget_visuals.inactive.fg_stroke.color);
            }

            add_content(ui)
        })
        .inner;

    let main_button_rect = main_button_response.rect;

    // We check the same rectangle as the main button,
    // but we will also catch hovers on the visibility button (if any).
    let button_hovered = ui
        .interact(main_button_rect, ui.id(), egui::Sense::hover())
        .hovered();
    if button_hovered {
        // Just put the buttons on top of the existing ui:
        let mut ui = ui.child_ui(
            ui.max_rect(),
            egui::Layout::right_to_left(egui::Align::Center),
        );
        add_on_hover_buttons(re_ui, &mut ui);
    }

    // The main button might have been highlighted because what it was referring
    // to was hovered somewhere else, and then we also want it highlighted here.
    if button_hovered || main_button_response.highlighted() {
        // Highlight the row:
        let visuals = ui.visuals().widgets.hovered;
        let hover_rect = main_button_rect.expand(visuals.expansion);
        ui.painter().set(
            where_to_add_hover_rect,
            egui::Shape::rect_filled(hover_rect, visuals.rounding, visuals.bg_fill),
        );
    }
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
        let space_view_state = self.viewport_state.space_view_state(
            self.ctx.space_view_class_registry,
            space_view_blueprint.id,
            space_view_blueprint.class,
        );

        space_view_ui(
            self.ctx,
            ui,
            space_view_blueprint,
            space_view_state,
            highlights,
        );

        Default::default()
    }

    fn tab_title_for_pane(&mut self, space_view_id: &SpaceViewId) -> egui::WidgetText {
        let space_view = self
            .space_views
            .get_mut(space_view_id)
            .expect("Should have been populated beforehand");

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
                    .set_single_selection(Item::SpaceView(*space_view_id));
            }
        }
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
        let Some(space_view_state) = self.viewport_state.space_view_states.get(&space_view_id) else { return; };

        let num_space_views = tiles.tiles.values().filter(|tile| tile.is_pane()).count();

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
                    .set_single_selection(Item::SpaceView(space_view_id));
            }
        }

        // Show help last, since not all space views have help text
        help_text_ui(self.ctx, ui, space_view, space_view_state);
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

fn help_text_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_blueprint: &SpaceViewBlueprint,
    space_view_state: &SpaceViewState,
) {
    let help_text = if let Ok(class) = ctx
        .space_view_class_registry
        .get(space_view_blueprint.class)
    {
        Some(class.help_text(ctx.re_ui, space_view_state.state.as_ref()))
    } else {
        match space_view_blueprint.category {
            ViewCategory::TimeSeries => Some(crate::view_time_series::help_text(ctx.re_ui)),
            ViewCategory::BarChart => Some(crate::view_bar_chart::help_text(ctx.re_ui)),
            ViewCategory::TextBox
            | ViewCategory::Text
            | ViewCategory::Tensor
            | ViewCategory::Spatial => None,
        }
    };

    if let Some(help_text) = help_text {
        re_ui::help_hover_button(ui).on_hover_text(help_text);
    }
}

fn space_view_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_blueprint: &mut SpaceViewBlueprint,
    space_view_state: &mut SpaceViewState,
    space_view_highlights: SpaceViewHighlights,
) {
    let Some(latest_at) = ctx.rec_cfg.time_ctrl.time_int() else {
        ui.centered_and_justified(|ui| {
            ui.weak("No time selected");
        });
        return
    };

    space_view_blueprint.scene_ui(space_view_state, ctx, ui, latest_at, space_view_highlights);
}

// ----------------------------------------------------------------------------

fn focus_tab(tree: &mut egui_tiles::Tree<SpaceViewId>, tab: &SpaceViewId) {
    tree.make_active(|tile| match tile {
        egui_tiles::Tile::Pane(space_view_id) => space_view_id == tab,
        egui_tiles::Tile::Container(_) => false,
    });
}
