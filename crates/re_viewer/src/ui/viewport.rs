//! The viewport panel.
//!
//! Contains all space views.

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_store::EntityPath;

use crate::{
    misc::{space_info::SpaceInfoCollection, Item, SpaceViewHighlights, ViewerContext},
    ui::space_view_heuristics::default_created_space_views,
};

use super::{
    data_blueprint::DataBlueprintGroupHandle, space_view_entity_picker::SpaceViewEntityPicker,
    space_view_heuristics::all_possible_space_views, view_category::ViewCategory, SpaceView,
    SpaceViewId,
};

// ----------------------------------------------------------------------------

/// What views are visible?
type VisibilitySet = std::collections::BTreeSet<SpaceViewId>;

/// Describes the layout and contents of the Viewport Panel.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Viewport {
    /// Where the space views are stored.
    space_views: HashMap<SpaceViewId, SpaceView>,

    /// Which views are visible.
    visible: VisibilitySet,

    /// The layouts of all the space views.
    ///
    /// One for each combination of what views are visible.
    /// So if a user toggles the visibility of one SpaceView, we
    /// switch which layout we are using. This is somewhat hacky.
    trees: HashMap<VisibilitySet, egui_dock::Tree<SpaceViewId>>,

    /// Show one tab as maximized?
    maximized: Option<SpaceViewId>,

    /// Set to `true` the first time the user messes around with the viewport blueprint.
    /// Before this is set we automatically add new spaces to the viewport
    /// when they show up in the data.
    has_been_user_edited: bool,

    #[serde(skip)]
    space_view_entity_window: Option<SpaceViewEntityPicker>,
}

impl Viewport {
    /// Create a default suggested blueprint using some heuristics.
    pub fn new(ctx: &mut ViewerContext<'_>, spaces_info: &SpaceInfoCollection) -> Self {
        crate::profile_function!();

        let mut blueprint = Self::default();
        for space_view in default_created_space_views(ctx, spaces_info) {
            blueprint.add_space_view(space_view);
        }
        blueprint
    }

    pub(crate) fn space_view(&self, space_view: &SpaceViewId) -> Option<&SpaceView> {
        self.space_views.get(space_view)
    }

    pub(crate) fn space_view_mut(&mut self, space_view_id: &SpaceViewId) -> Option<&mut SpaceView> {
        self.space_views.get_mut(space_view_id)
    }

    pub(crate) fn remove(&mut self, space_view_id: &SpaceViewId) -> Option<SpaceView> {
        let Self {
            space_views,
            visible,
            trees,
            maximized,
            has_been_user_edited,
            space_view_entity_window,
        } = self;

        if let Some(window) = space_view_entity_window {
            if window.space_view_id == *space_view_id {
                *space_view_entity_window = None;
            }
        }

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
        crate::profile_function!();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let space_view_ids = self
                    .space_views
                    .keys()
                    .sorted_by_key(|space_view_id| &self.space_views[space_view_id].space_path)
                    .copied()
                    .collect_vec();

                for space_view_id in &space_view_ids {
                    self.space_view_entry_ui(ctx, ui, space_view_id);
                }
            });
    }

    // If a group or spaceview has a total of this number of elements or less, show its subtree by default.
    const MAX_ELEM_FOR_DEFAULT_OPEN: usize = 3;

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
        let default_open = root_group.children.len() + root_group.entities.len()
            <= Self::MAX_ELEM_FOR_DEFAULT_OPEN;
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
                    let response = ctx.space_view_button(ui, space_view);
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
        space_view: &mut SpaceView,
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
                        ctx.data_blueprint_button_to(ui, label, space_view.id, entity_path)
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
                            .on_hover_text("Remove group and all its children from the space view.")
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
            let default_open = child_group.children.len() + child_group.entities.len()
                <= Self::MAX_ELEM_FOR_DEFAULT_OPEN;
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
                        ctx.data_blueprint_group_button_to(
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
                            .on_hover_text("Remove Entity from the space view.")
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

    pub(crate) fn mark_user_interaction(&mut self) {
        self.has_been_user_edited = true;
    }

    pub(crate) fn add_space_view(&mut self, space_view: SpaceView) -> SpaceViewId {
        let id = space_view.id;
        self.space_views.insert(id, space_view);
        self.visible.insert(id);
        self.trees.clear(); // Reset them
        id
    }

    pub fn show_add_remove_entities_window(&mut self, space_view_id: SpaceViewId) {
        self.space_view_entity_window = Some(SpaceViewEntityPicker { space_view_id });
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        crate::profile_function!();

        for space_view in self.space_views.values_mut() {
            space_view.on_frame_start(ctx, spaces_info);
        }

        if !self.has_been_user_edited {
            for space_view_candidate in default_created_space_views(ctx, spaces_info) {
                if self.should_auto_add_space_view(&space_view_candidate) {
                    self.add_space_view(space_view_candidate);
                }
            }
        }
    }

    fn should_auto_add_space_view(&self, space_view_candidate: &SpaceView) -> bool {
        for existing_view in self.space_views.values() {
            if existing_view.space_path == space_view_candidate.space_path {
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

    pub fn viewport_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewerContext<'_>) {
        if let Some(window) = &mut self.space_view_entity_window {
            if let Some(space_view) = self.space_views.get_mut(&window.space_view_id) {
                if !window.ui(ctx, ui, space_view) {
                    self.space_view_entity_window = None;
                }
            } else {
                // The space view no longer exist, close the window!
                self.space_view_entity_window = None;
            }
        }

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return;
        }

        self.trees.retain(|_, tree| is_tree_valid(tree));

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
                    ui.available_size(),
                    &visible_space_views,
                    &self.space_views,
                )
            });

        let num_space_views = tree.num_tabs();
        if num_space_views == 0 {
            return;
        }

        let mut tab_viewer = TabViewer {
            ctx,
            space_views: &mut self.space_views,
        };

        ui.scope(|ui| {
            // we need a scope, because egui_dock unfortunately messes with the ui clip rect

            ui.spacing_mut().item_spacing.x = re_ui::ReUi::view_padding();

            egui_dock::DockArea::new(tree)
                .style(re_ui::egui_dock_style(ui.style()))
                .show_inside(ui, &mut tab_viewer);
        });

        // Two passes so we avoid borrowing issues:
        let tab_bars = tree
            .iter()
            .filter_map(|node| {
                let egui_dock::Node::Leaf {
                        rect,
                        viewport,
                        tabs,
                        active,
                    } = node else {
                        return None;
                    };

                let space_view_id = tabs.get(active.0)?;

                // `rect` includes the tab area, while `viewport` is just the tab body.
                // so the tab bar rect is:
                let tab_bar_rect =
                    egui::Rect::from_x_y_ranges(rect.x_range(), rect.top()..=viewport.top());

                // rect/viewport can be invalid for the first frame
                tab_bar_rect
                    .is_finite()
                    .then_some((*space_view_id, tab_bar_rect))
            })
            .collect_vec();

        for (space_view_id, tab_bar_rect) in tab_bars {
            // rect/viewport can be invalid for the first frame
            space_view_options_ui(ctx, ui, self, tab_bar_rect, space_view_id, num_space_views);
        }
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
                .sorted_by_key(|space_view| space_view.space_path.to_string())
            {
                if ctx
                    .re_ui
                    .selectable_label_with_icon(
                        ui,
                        space_view.category.icon(),
                        if space_view.space_path.is_root() {
                            space_view.display_name.clone()
                        } else {
                            space_view.space_path.to_string()
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
    ctx: &'a mut ViewerContext<'b>,
    space_views: &'a mut HashMap<SpaceViewId, SpaceView>,
}

impl<'a, 'b> egui_dock::TabViewer for TabViewer<'a, 'b> {
    type Tab = SpaceViewId;

    fn ui(&mut self, ui: &mut egui::Ui, space_view_id: &mut Self::Tab) {
        crate::profile_function!();

        let highlights = self
            .ctx
            .selection_state()
            .highlights_for_space_view(*space_view_id, self.space_views);
        let space_view = self
            .space_views
            .get_mut(space_view_id)
            .expect("Should have been populated beforehand");

        space_view_ui(self.ctx, ui, space_view, &highlights);
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let space_view = self
            .space_views
            .get_mut(tab)
            .expect("Should have been populated beforehand");

        let mut text =
            egui::WidgetText::RichText(egui::RichText::new(space_view.display_name.clone()));

        if self.ctx.selection().contains(&Item::SpaceView(*tab)) {
            // Show that it is selected:
            let egui_ctx = &self.ctx.re_ui.egui_ctx;
            let selection_bg_color = egui_ctx.style().visuals.selection.bg_fill;
            text = text.background_color(selection_bg_color);
        }

        text
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        if response.clicked() {
            self.ctx.set_single_selection(Item::SpaceView(*tab));
        }
    }
}

fn help_text_ui(ui: &mut egui::Ui, space_view: &SpaceView) {
    let help_text = match space_view.category {
        ViewCategory::TimeSeries => Some(crate::ui::view_time_series::HELP_TEXT),
        ViewCategory::BarChart => Some(crate::ui::view_bar_chart::HELP_TEXT),
        ViewCategory::Spatial => Some(space_view.view_state.state_spatial.help_text()),
        ViewCategory::Text | ViewCategory::Tensor => None,
    };

    if let Some(help_text) = help_text {
        crate::misc::help_hover_button(ui).on_hover_text(help_text);
    }
}

/// Shown in the right of the tab panel
fn space_view_options_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    viewport: &mut Viewport,
    tab_bar_rect: egui::Rect,
    space_view_id: SpaceViewId,
    num_space_views: usize,
) {
    let Some(space_view) = viewport.space_views.get(&space_view_id) else { return; };

    let tab_bar_rect = tab_bar_rect.shrink2(egui::vec2(4.0, 0.0)); // Add some side margin outside the frame

    ui.allocate_ui_at_rect(tab_bar_rect, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let where_to_put_background = ui.painter().add(egui::Shape::Noop);

            ui.add_space(4.0); // margin within the frame

            if viewport.maximized == Some(space_view_id) {
                // Show minimize-button:
                if ctx
                    .re_ui
                    .small_icon_button(ui, &re_ui::icons::MINIMIZE)
                    .on_hover_text("Restore - show all spaces")
                    .clicked()
                {
                    viewport.maximized = None;
                }
            } else if num_space_views > 1 {
                // Show maximize-button:
                if ctx
                    .re_ui
                    .small_icon_button(ui, &re_ui::icons::MAXIMIZE)
                    .on_hover_text("Maximize Space View")
                    .clicked()
                {
                    viewport.maximized = Some(space_view_id);
                    ctx.set_single_selection(Item::SpaceView(space_view_id));
                }
            }

            // Show help last, since not all space views have help text
            help_text_ui(ui, space_view);

            // Put a frame so that the buttons cover any labels they intersect with:
            let rect = ui.min_rect().expand2(egui::vec2(1.0, -2.0));
            ui.painter().set(
                where_to_put_background,
                egui::Shape::rect_filled(
                    rect,
                    0.0,
                    re_ui::egui_dock_style(ui.style()).tab_bar_background_color,
                ),
            );
        });
    });
}

fn space_view_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view: &mut SpaceView,
    space_view_highlights: &SpaceViewHighlights,
) {
    let Some(latest_at) = ctx.rec_cfg.time_ctrl.time_int() else {
        ui.centered_and_justified(|ui| {
            ui.label(ctx.re_ui.warning_text("No time selected"));
        });
        return
    };

    space_view.scene_ui(ctx, ui, latest_at, space_view_highlights);
}

// ----------------------------------------------------------------------------

fn focus_tab(tree: &mut egui_dock::Tree<SpaceViewId>, tab: &SpaceViewId) {
    if let Some((node_index, tab_index)) = tree.find_tab(tab) {
        tree.set_focused_node(node_index);
        tree.set_active_tab(node_index, tab_index);
    }
}

fn is_tree_valid(tree: &egui_dock::Tree<SpaceViewId>) -> bool {
    tree.iter().all(|node| match node {
        egui_dock::Node::Vertical { rect: _, fraction }
        | egui_dock::Node::Horizontal { rect: _, fraction } => fraction.is_finite(),
        _ => true,
    })
}
