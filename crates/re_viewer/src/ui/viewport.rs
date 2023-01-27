//! The viewport panel.
//!
//! Contains all space views.

use std::collections::BTreeSet;

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_store::{ObjPath, TimeInt};
use re_log_types::field_types::{Tensor, TensorTrait};

use crate::misc::{space_info::SpaceInfoCollection, Selection, SpaceViewHighlights, ViewerContext};

use super::{
    data_blueprint::{DataBlueprintGroupHandle, DataBlueprintTree},
    view_category::ViewCategory,
    SpaceView, SpaceViewId,
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
}

impl Viewport {
    /// Create a default suggested blueprint using some heuristics.
    pub fn new(ctx: &mut ViewerContext<'_>, spaces_info: &SpaceInfoCollection) -> Self {
        crate::profile_function!();

        let mut blueprint = Self::default();
        for space_view in Self::default_created_space_views(ctx, spaces_info) {
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
        crate::profile_function!();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let space_view_ids = self
                    .space_views
                    .keys()
                    .sorted_by_key(|space_view_id| &self.space_views[space_view_id].name)
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
        let space_view = self.space_views.get_mut(space_view_id).unwrap();
        debug_assert_eq!(space_view.id, *space_view_id);

        let root_group = space_view.data_blueprint.root_group();
        let default_open =
            root_group.children.len() + root_group.objects.len() <= Self::MAX_ELEM_FOR_DEFAULT_OPEN;
        let collapsing_header_id = ui.id().with(space_view.id);
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            ui.label(space_view.category.icon());

            if ctx
                .space_view_button_to(ui, space_view.name.clone(), *space_view_id)
                .clicked()
            {
                if let Some(tree) = self.trees.get_mut(&self.visible) {
                    focus_tab(tree, space_view_id);
                }
            }

            let mut is_space_view_visible = self.visible.contains(space_view_id);
            if visibility_button(ui, true, &mut is_space_view_visible).changed() {
                self.has_been_user_edited = true;
                if is_space_view_visible {
                    self.visible.insert(*space_view_id);
                } else {
                    self.visible.remove(space_view_id);
                }
            }
        })
        .body(|ui| {
            Self::data_blueprint_tree_ui(
                ctx,
                ui,
                space_view.data_blueprint.root_handle(),
                &mut space_view.data_blueprint,
                space_view_id,
                self.visible.contains(space_view_id),
            );
        });
    }

    fn data_blueprint_tree_ui(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        group_handle: DataBlueprintGroupHandle,
        data_blueprint_tree: &mut DataBlueprintTree,
        space_view_id: &SpaceViewId,
        space_view_visible: bool,
    ) {
        let Some(group) = data_blueprint_tree.group(group_handle) else {
            debug_assert!(false, "Invalid group handle in blueprint group tree");
            return;
        };

        // TODO(andreas): These clones are workarounds against borrowing multiple times from data_blueprint_tree.
        let children = group.children.clone();
        let objects = group.objects.clone();
        let group_name = group.display_name.clone();
        let group_is_visible = group.properties_projected.visible && space_view_visible;

        for path in &objects {
            ui.horizontal(|ui| {
                let name = path.iter().last().unwrap().to_string();

                ctx.data_blueprint_button_to(ui, name, *space_view_id, path);

                let mut properties = data_blueprint_tree.data_blueprints_individual().get(path);
                if visibility_button(ui, group_is_visible, &mut properties.visible).changed() {
                    data_blueprint_tree
                        .data_blueprints_individual()
                        .set(path.clone(), properties);
                }
            });
        }

        for child_group_handle in &children {
            let Some(child_group) = data_blueprint_tree.group_mut(*child_group_handle) else {
                debug_assert!(false, "Data blueprint group {group_name} has an invalid child");
                continue;
            };

            let default_open = child_group.children.len() + child_group.objects.len()
                <= Self::MAX_ELEM_FOR_DEFAULT_OPEN;
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(child_group_handle),
                default_open,
            )
            .show_header(ui, |ui| {
                ui.label("📁");
                ctx.data_blueprint_group_button_to(
                    ui,
                    &child_group.display_name,
                    *space_view_id,
                    *child_group_handle,
                );
                visibility_button(
                    ui,
                    group_is_visible,
                    &mut child_group.properties_individual.visible,
                );
            })
            .body(|ui| {
                Self::data_blueprint_tree_ui(
                    ctx,
                    ui,
                    *child_group_handle,
                    data_blueprint_tree,
                    space_view_id,
                    space_view_visible,
                );
            });
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
            for space_view_candidate in Self::default_created_space_views(ctx, spaces_info) {
                if self.should_auto_add_space_view(&space_view_candidate) {
                    self.add_space_view(space_view_candidate);
                }
            }
        }
    }

    fn should_auto_add_space_view(&self, space_view_candidate: &SpaceView) -> bool {
        for existing_view in self.space_views.values() {
            if existing_view.space_path == space_view_candidate.space_path {
                if !existing_view.allow_auto_adding_more_object {
                    // Since the user edited a space view with the same space path, we can't be sure our new one isn't redundant.
                    // So let's skip that.
                    return false;
                }

                if space_view_candidate
                    .data_blueprint
                    .object_paths()
                    .is_subset(existing_view.data_blueprint.object_paths())
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
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
        selection_panel_expanded: &mut bool,
    ) {
        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return;
        }

        self.trees.retain(|_, tree| is_tree_valid(tree));

        // Lazily create a layout tree based on which SpaceViews are currently visible:
        let tree = self.trees.entry(self.visible.clone()).or_insert_with(|| {
            super::auto_layout::tree_from_space_views(
                ui.available_size(),
                &self.visible,
                &self.space_views,
            )
        });

        let num_space_views = tree.num_tabs();
        if num_space_views == 0 {
            // nothing to show
        } else if num_space_views == 1 {
            let space_view_id = *tree.tabs().next().unwrap();
            let highlights = ctx
                .selection_state()
                .highlights_for_space_view(space_view_id, &self.space_views);
            let space_view = self
                .space_views
                .get_mut(&space_view_id)
                .expect("Should have been populated beforehand");
            let response = ui
                .scope(|ui| space_view_ui(ctx, ui, spaces_info, space_view, &highlights))
                .response;

            let frame = ctx.re_ui.hovering_frame();
            hovering_panel(ui, frame, response.rect, |ui| {
                space_view_options_link(ctx, selection_panel_expanded, space_view.id, ui, "⛭");
                help_text_ui(ui, space_view);
            });
        } else if let Some(space_view_id) = self.maximized {
            let highlights = ctx
                .selection_state()
                .highlights_for_space_view(space_view_id, &self.space_views);
            let space_view = self
                .space_views
                .get_mut(&space_view_id)
                .expect("Should have been populated beforehand");
            let response = ui
                .scope(|ui| space_view_ui(ctx, ui, spaces_info, space_view, &highlights))
                .response;

            let frame = ctx.re_ui.hovering_frame();
            hovering_panel(ui, frame, response.rect, |ui| {
                if ctx
                    .re_ui
                    .small_icon(ui, &re_ui::icons::MINIMIZE)
                    .on_hover_text("Restore - show all spaces")
                    .clicked()
                {
                    self.maximized = None;
                }
                space_view_options_link(ctx, selection_panel_expanded, space_view.id, ui, "⛭");
                help_text_ui(ui, space_view);
            });
        } else {
            let mut dock_style = egui_dock::Style::from_egui(ui.style().as_ref());
            dock_style.separator_width = 2.0;
            dock_style.default_inner_margin = 0.0.into();
            dock_style.show_close_buttons = false;
            dock_style.tab_include_scrollarea = false;
            // dock_style.expand_tabs = true; looks good, but decreases readability
            dock_style.tab_text_color_unfocused = dock_style.tab_text_color_focused; // We don't treat focused tabs differently
            dock_style.tab_background_color = ui.visuals().panel_fill;

            let mut tab_viewer = TabViewer {
                ctx,
                spaces_info,
                space_views: &mut self.space_views,
                maximized: &mut self.maximized,
                selection_panel_expanded,
            };

            egui_dock::DockArea::new(tree)
                .style(dock_style)
                .show_inside(ui, &mut tab_viewer);
        }
    }

    fn all_possible_space_views(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) -> Vec<SpaceView> {
        crate::profile_function!();

        let mut space_views = Vec::new();

        for space_info in spaces_info.iter() {
            for (category, obj_paths) in
                SpaceView::default_queried_objects_by_category(ctx, spaces_info, space_info)
            {
                space_views.push(SpaceView::new(category, space_info, &obj_paths));
            }
        }

        space_views
    }

    fn default_created_space_views(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) -> Vec<SpaceView> {
        crate::profile_function!();

        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let timeline_query = re_arrow_store::LatestAtQuery::new(*timeline, TimeInt::from(i64::MAX));

        let mut space_views = Vec::new();

        for space_view_candidate in Self::all_possible_space_views(ctx, spaces_info) {
            let Some(space_info) = spaces_info.get(&space_view_candidate.space_path) else {
                // Should never happen.
                continue;
            };

            // If it doesn't contain anything but the transform itself, skip,
            if space_info.descendants_without_transform.is_empty() {
                continue;
            }

            if space_view_candidate.category == ViewCategory::Spatial {
                // Skip if connection to parent is via rigid (too trivial for a new space view!)
                if let Some(parent_transform) = space_info.parent_transform() {
                    match parent_transform {
                        re_log_types::Transform::Rigid3(_) => {
                            continue;
                        }
                        re_log_types::Transform::Pinhole(_) | re_log_types::Transform::Unknown => {}
                    }
                }

                // Gather all images that are untransformed children of the space view candidate's root.
                let images = space_info
                    .descendants_without_transform
                    .iter()
                    .filter_map(|obj_path| {
                        if let Ok(entity_view) = re_query::query_entity_with_primary::<Tensor>(
                            &ctx.log_db.obj_db.arrow_store,
                            &timeline_query,
                            obj_path,
                            &[],
                        ) {
                            if let Ok(iter) = entity_view.iter_primary() {
                                for tensor in iter.flatten() {
                                    if tensor.is_shaped_like_an_image() {
                                        return Some((obj_path.clone(), tensor.shape));
                                    }
                                }
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>();

                if images.len() > 1 {
                    // Multiple images (e.g. depth and rgb, or rgb and segmentation) in the same 2D scene.
                    // Stacking them on top of each other works, but is often confusing.
                    // Let's create one space view for each image, where the other images are disabled:

                    let mut image_sizes = BTreeSet::default();
                    for (obj_path, shape) in &images {
                        debug_assert!(matches!(shape.len(), 2 | 3));
                        let image_size = (shape[0].size, shape[1].size);
                        image_sizes.insert(image_size);

                        // Space view with everything but the other images.
                        // (note that other objects stay!)
                        let mut single_image_space_view = space_view_candidate.clone();
                        for (other_obj_path, _) in &images {
                            if other_obj_path != obj_path {
                                single_image_space_view
                                    .data_blueprint
                                    .remove_object(other_obj_path);
                            }
                        }
                        single_image_space_view.allow_auto_adding_more_object = false;
                        space_views.push(single_image_space_view);
                    }

                    // Only if all images have the same size, so we _also_ want to create the stacked version (e.g. rgb + segmentation)
                    // TODO(andreas): What if there's also other objects that we want to show?
                    if image_sizes.len() > 1 {
                        continue;
                    }
                }
            }

            space_views.push(space_view_candidate);
        }

        space_views
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

            for space_view in Self::all_possible_space_views(ctx, spaces_info) {
                if ui
                    .button(format!(
                        "{} {}",
                        space_view.category.icon(),
                        space_view.name
                    ))
                    .clicked()
                {
                    ui.close_menu();
                    let new_space_view_id = self.add_space_view(space_view);
                    ctx.set_single_selection(Selection::SpaceView(new_space_view_id));
                }
            }
        })
        .response
        .on_hover_text("Add new space view.");
    }

    pub fn space_views_containing_obj_path(&self, path: &ObjPath) -> Vec<SpaceViewId> {
        self.space_views
            .iter()
            .filter_map(|(space_view_id, space_view)| {
                if space_view.data_blueprint.contains_object(path) {
                    Some(*space_view_id)
                } else {
                    None
                }
            })
            .collect()
    }
}

fn visibility_button(ui: &mut egui::Ui, enabled: bool, visible: &mut bool) -> egui::Response {
    ui.add_space(16.0); // Make room for visibility button so the side bar don't become too narrow to fit it

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_enabled(enabled);
        if enabled {
            ui.toggle_value(visible, "👁")
        } else {
            let mut always_false = false;
            ui.toggle_value(&mut always_false, "👁")
        }
        .on_hover_text("Toggle visibility")
    })
    .inner
}

// ----------------------------------------------------------------------------

struct TabViewer<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    spaces_info: &'a SpaceInfoCollection,
    space_views: &'a mut HashMap<SpaceViewId, SpaceView>,
    maximized: &'a mut Option<SpaceViewId>,
    selection_panel_expanded: &'a mut bool,
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

        let response = ui
            .scope(|ui| space_view_ui(self.ctx, ui, self.spaces_info, space_view, &highlights))
            .response;

        // Show buttons for maximize and space view options:
        let frame = self.ctx.re_ui.hovering_frame();
        hovering_panel(ui, frame, response.rect, |ui| {
            if self
                .ctx
                .re_ui
                .small_icon(ui, &re_ui::icons::MAXIMIZE)
                .on_hover_text("Maximize Space View")
                .clicked()
            {
                *self.maximized = Some(*space_view_id);
                self.ctx
                    .set_single_selection(Selection::SpaceView(*space_view_id));
            }

            space_view_options_link(
                self.ctx,
                self.selection_panel_expanded,
                *space_view_id,
                ui,
                "⛭",
            );

            help_text_ui(ui, space_view);
        });
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let space_view = self
            .space_views
            .get_mut(tab)
            .expect("Should have been populated beforehand");
        space_view.name.clone().into()
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

fn space_view_options_link(
    ctx: &mut ViewerContext<'_>,
    selection_panel_expanded: &mut bool,
    space_view_id: SpaceViewId,
    ui: &mut egui::Ui,
    text: &str,
) {
    let selection = Selection::SpaceView(space_view_id);
    let is_selected = ctx.selection().contains(&selection) && *selection_panel_expanded;
    if ui
        .selectable_label(is_selected, text)
        .on_hover_text("Space View options")
        .clicked()
    {
        if is_selected {
            ctx.selection_state_mut().clear_current();
            *selection_panel_expanded = false;
        } else {
            ctx.set_single_selection(selection);
            *selection_panel_expanded = true;
        }
    }
}

fn hovering_panel(
    ui: &mut egui::Ui,
    frame: egui::Frame,
    rect: egui::Rect,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let mut ui = ui.child_ui(rect, egui::Layout::top_down(egui::Align::LEFT));
    ui.horizontal(|ui| {
        frame.show(ui, add_contents);
    });
}

fn space_view_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
    space_view: &mut SpaceView,
    space_view_highlights: &SpaceViewHighlights,
) {
    let Some(reference_space_info) = spaces_info.get(&space_view.space_path) else {
        ui.centered_and_justified(|ui| {
            ui.label(ctx.re_ui.warning_text(format!("Unknown space {}", space_view.space_path)));
        });
        return;
    };

    let Some(latest_at) = ctx.rec_cfg.time_ctrl.time_int() else {
        ui.centered_and_justified(|ui| {
            ui.label(ctx.re_ui.warning_text("No time selected"));
        });
        return
    };

    space_view.scene_ui(
        ctx,
        ui,
        reference_space_info,
        latest_at,
        space_view_highlights,
    );
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
