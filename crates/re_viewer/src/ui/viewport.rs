//! The viewport panel.
//!
//! Contains all space views.
//!
//! To do:
//! * [ ] Opening up new Space Views
//! * [ ] Controlling visibility of objects inside each Space View
//! * [ ] Transforming objects between spaces

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, TimeInt};

use crate::misc::{
    space_info::{SpaceInfo, SpacesInfo},
    Selection, ViewerContext,
};

use super::{space_view::ViewCategory, SceneQuery, SpaceView};

// ----------------------------------------------------------------------------

/// A unique id for each space view.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
pub struct SpaceViewId(uuid::Uuid);

impl SpaceViewId {
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

// ----------------------------------------------------------------------------

fn query_scene(ctx: &mut ViewerContext<'_>, space_info: &SpaceInfo) -> super::scene::Scene {
    let query = SceneQuery {
        obj_paths: &space_info.objects,
        timeline: *ctx.rec_cfg.time_ctrl.timeline(),
        latest_at: TimeInt::MAX,
        obj_props: &Default::default(), // all visible
    };
    query.query(ctx)
}

// ----------------------------------------------------------------------------

/// What views are visible?
type VisibilitySet = std::collections::BTreeSet<SpaceViewId>;

/// Describes the layout and contents of the Viewport Panel.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewportBlueprint {
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

impl ViewportBlueprint {
    /// Create a default suggested blueprint using some heuristics.
    fn new(ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) -> Self {
        crate::profile_function!();

        let mut blueprint = Self::default();

        for (path, space_info) in &spaces_info.spaces {
            let scene = query_scene(ctx, space_info);
            for category in scene.categories() {
                if category == ViewCategory::Spatial
                    && scene.spatial.prefer_2d_mode()
                    && scene.spatial.ui.images.len() > 1
                {
                    // Multiple images (e.g. depth and rgb, or rgb and segmentation) in the same 2D scene.
                    // Stacking them on top of each other works, but is often confusing.
                    // Let's create one space view for each image, where the other images are disabled:

                    let store = &ctx.log_db.obj_db.store;

                    for visible_image in &scene.spatial.ui.images {
                        if let Some(visible_instance_id) =
                            visible_image.instance_hash.resolve(store)
                        {
                            let mut space_view = SpaceView::new(&scene, category, path.clone());
                            space_view.name = visible_instance_id.obj_path.to_string();

                            for other_image in &scene.spatial.ui.images {
                                if let Some(image_instance_id) =
                                    other_image.instance_hash.resolve(store)
                                {
                                    let visible =
                                        visible_instance_id.obj_path == image_instance_id.obj_path;

                                    space_view.obj_tree_properties.individual.set(
                                        image_instance_id.obj_path,
                                        re_data_store::ObjectProps {
                                            visible,
                                            ..Default::default()
                                        },
                                    );
                                }
                            }

                            let space_view_id = SpaceViewId::random();
                            blueprint.space_views.insert(space_view_id, space_view);
                            blueprint.visible.insert(space_view_id);
                        }
                    }

                    // We _also_ want to create the stacked version, e.g. rgb + segmentation
                    // so we keep going here.
                }

                // Create one SpaceView for the whole space:
                {
                    let space_view = SpaceView::new(&scene, category, path.clone());
                    let space_view_id = SpaceViewId::random();
                    blueprint.space_views.insert(space_view_id, space_view);
                    blueprint.visible.insert(space_view_id);
                }
            }
        }

        // Make sure the visibility flags in the SpaceView::obj_tree_properties get updated:
        for space_view in blueprint.space_views.values_mut() {
            space_view.on_frame_start(&ctx.log_db.obj_db.tree);
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

    fn has_space(&self, space_path: &ObjPath) -> bool {
        self.space_views
            .values()
            .any(|view| &view.reference_space_path == space_path)
    }

    /// Show the blueprint panel tree view.
    fn tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        obj_tree: &ObjectTree,
    ) {
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
                    self.space_view_tree_ui(ctx, ui, spaces_info, obj_tree, space_view_id);
                }
            });
    }

    fn space_view_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        obj_tree: &ObjectTree,
        space_view_id: &SpaceViewId,
    ) {
        let space_view = self.space_views.get_mut(space_view_id).unwrap();

        let collapsing_header_id = ui.make_persistent_id(space_view_id);

        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            false,
        )
        .show_header(ui, |ui| {
            match space_view.category {
                ViewCategory::Spatial => match space_view.view_state.state_spatial.nav_mode {
                    super::view_spatial::SpatialNavigationMode::TwoD => ui.label("🖼"),
                    super::view_spatial::SpatialNavigationMode::ThreeD => ui.label("🔭"),
                },
                ViewCategory::Tensor => ui.label("🇹"),
                ViewCategory::Text => ui.label("📃"),
                ViewCategory::Plot => ui.label("📈"),
            };

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
            let is_space_view_visible = self.visible.contains(space_view_id);
            show_obj_tree_children(
                ctx,
                ui,
                is_space_view_visible,
                &mut space_view.obj_tree_properties,
                *space_view_id,
                spaces_info,
                &mut space_view.reference_space_path,
                obj_tree,
            );
        });
    }

    pub(crate) fn mark_user_interaction(&mut self) {
        self.has_been_user_edited = true;
    }

    pub(crate) fn add_space_view(&mut self, space_view: SpaceView) -> SpaceViewId {
        let space_view_id = SpaceViewId::random();
        self.space_views.insert(space_view_id, space_view);
        self.visible.insert(space_view_id);
        self.trees.clear(); // Reset them
        space_view_id
    }

    fn add_space_view_for(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        path: &ObjPath,
        space_info: &SpaceInfo,
    ) {
        let scene = query_scene(ctx, space_info);
        for category in scene.categories() {
            self.add_space_view(SpaceView::new(&scene, category, path.clone()));
        }
    }

    fn on_frame_start(&mut self, ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) {
        crate::profile_function!();

        if !self.has_been_user_edited {
            // Automatically populate the viewport based on the data:

            if self.space_views.is_empty() {
                *self = Self::new(ctx, spaces_info);
            } else {
                crate::profile_scope!("look for missing space views");

                // Check if the blueprint is missing a space,
                // maybe one that has been added by new data:
                for (path, space_info) in &spaces_info.spaces {
                    if !self.has_space(path) {
                        self.add_space_view_for(ctx, path, space_info);
                    }
                }
            }
        }

        for space_view in self.space_views.values_mut() {
            space_view.on_frame_start(&ctx.log_db.obj_db.tree);
        }
    }

    fn viewport_ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpacesInfo,
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
            let space_view = self
                .space_views
                .get_mut(&space_view_id)
                .expect("Should have been populated beforehand");

            let response = ui
                .scope(|ui| space_view_ui(ctx, ui, spaces_info, space_view))
                .response;

            let frame = ctx.re_ui.hovering_frame();
            hovering_panel(ui, frame, response.rect, |ui| {
                space_view_options_link(ctx, selection_panel_expanded, space_view_id, ui, "⛭");
            });
        } else if let Some(space_view_id) = self.maximized {
            let space_view = self
                .space_views
                .get_mut(&space_view_id)
                .expect("Should have been populated beforehand");

            let response = ui
                .scope(|ui| space_view_ui(ctx, ui, spaces_info, space_view))
                .response;

            let frame = ctx.re_ui.hovering_frame();
            hovering_panel(ui, frame, response.rect, |ui| {
                if ui
                    .button("⬅")
                    .on_hover_text("Restore - show all spaces")
                    .clicked()
                {
                    self.maximized = None;
                }
                space_view_options_link(ctx, selection_panel_expanded, space_view_id, ui, "⛭");
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

    fn create_new_blueprint_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
    ) {
        ui.vertical_centered(|ui| {
            ui.menu_button("Add new space view…", |ui| {
                ui.style_mut().wrap = Some(false);
                for (path, space_info) in &spaces_info.spaces {
                    let scene = query_scene(ctx, space_info);
                    if !scene.categories().is_empty() && ui.button(path.to_string()).clicked() {
                        ui.close_menu();

                        for category in scene.categories() {
                            let new_space_view_id =
                                self.add_space_view(SpaceView::new(&scene, category, path.clone()));
                            ctx.set_selection(Selection::SpaceView(new_space_view_id));
                        }
                    }
                }
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn show_obj_tree(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    parent_is_visible: bool,
    obj_tree_properties: &mut ObjectTreeProperties,
    space_view_id: SpaceViewId,
    spaces_info: &SpacesInfo,
    current_reference_frame: &mut ObjPath,
    name: &str,
    tree: &ObjectTree,
) {
    if tree.is_leaf() {
        ui.horizontal(|ui| {
            object_path_button(
                ctx,
                ui,
                &tree.path,
                space_view_id,
                spaces_info,
                current_reference_frame,
                &name,
            );
            object_visibility_button(ui, parent_is_visible, obj_tree_properties, &tree.path);
        });
    } else {
        let collapsing_header_id = ui.id().with(&tree.path);
        let default_open = false;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            object_path_button(
                ctx,
                ui,
                &tree.path,
                space_view_id,
                spaces_info,
                current_reference_frame,
                name,
            );
            object_visibility_button(ui, parent_is_visible, obj_tree_properties, &tree.path);
        })
        .body(|ui| {
            show_obj_tree_children(
                ctx,
                ui,
                parent_is_visible,
                obj_tree_properties,
                space_view_id,
                spaces_info,
                current_reference_frame,
                tree,
            );
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn show_obj_tree_children(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    parent_is_visible: bool,
    obj_tree_properties: &mut ObjectTreeProperties,
    space_view_id: SpaceViewId,
    spaces_info: &SpacesInfo,
    current_reference_frame: &mut ObjPath,
    tree: &ObjectTree,
) {
    if tree.children.is_empty() {
        ui.weak("(nothing)");
        return;
    }

    for (path_comp, child) in &tree.children {
        show_obj_tree(
            ctx,
            ui,
            parent_is_visible,
            obj_tree_properties,
            space_view_id,
            spaces_info,
            current_reference_frame,
            &path_comp.to_string(),
            child,
        );
    }
}

fn object_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    path: &ObjPath,
    space_view_id: SpaceViewId,
    spaces_info: &SpacesInfo,
    current_reference_frame: &mut ObjPath,
    name: &str,
) {
    let mut is_space_info = false;
    let label_text = if spaces_info.spaces.contains_key(&path) {
        is_space_info = true;
        let label_text = egui::RichText::new(format!("📐 {}", name));
        if path == current_reference_frame {
            label_text.strong()
        } else {
            label_text
        }
    } else {
        egui::RichText::new(name)
    };

    if ctx
        .space_view_obj_path_button_to(ui, label_text, space_view_id, path)
        .double_clicked()
        && is_space_info
    {
        *current_reference_frame = path.clone();
    }
}

fn object_visibility_button(
    ui: &mut egui::Ui,
    parent_is_visible: bool,
    obj_tree_properties: &mut ObjectTreeProperties,
    path: &ObjPath,
) {
    let are_all_ancestors_visible = parent_is_visible
        && match path.parent() {
            None => true, // root
            Some(parent) => obj_tree_properties.projected.get(&parent).visible,
        };

    let mut props = obj_tree_properties.individual.get(path);

    if visibility_button(ui, are_all_ancestors_visible, &mut props.visible).changed() {
        obj_tree_properties.individual.set(path.clone(), props);
    }
}

fn visibility_button(ui: &mut egui::Ui, enabled: bool, visible: &mut bool) -> egui::Response {
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
    spaces_info: &'a SpacesInfo,
    space_views: &'a mut HashMap<SpaceViewId, SpaceView>,
    maximized: &'a mut Option<SpaceViewId>,
    selection_panel_expanded: &'a mut bool,
}

impl<'a, 'b> egui_dock::TabViewer for TabViewer<'a, 'b> {
    type Tab = SpaceViewId;

    fn ui(&mut self, ui: &mut egui::Ui, space_view_id: &mut Self::Tab) {
        crate::profile_function!();

        let space_view = self
            .space_views
            .get_mut(space_view_id)
            .expect("Should have been populated beforehand");

        let response = ui
            .scope(|ui| space_view_ui(self.ctx, ui, self.spaces_info, space_view))
            .response;

        // Show buttons for maximize and space view options:
        let frame = self.ctx.re_ui.hovering_frame();
        hovering_panel(ui, frame, response.rect, |ui| {
            if ui
                .button("🗖")
                .on_hover_text("Maximize Space View")
                .clicked()
            {
                *self.maximized = Some(*space_view_id);
            }

            space_view_options_link(
                self.ctx,
                self.selection_panel_expanded,
                *space_view_id,
                ui,
                "⛭",
            );
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

fn space_view_options_link(
    ctx: &mut ViewerContext<'_>,
    selection_panel_expanded: &mut bool,
    space_view_id: SpaceViewId,
    ui: &mut egui::Ui,
    text: &str,
) {
    let is_selected =
        ctx.selection() == Selection::SpaceView(space_view_id) && *selection_panel_expanded;
    if ui
        .selectable_label(is_selected, text)
        .on_hover_text("Space View options")
        .clicked()
    {
        if is_selected {
            ctx.clear_selection();
            *selection_panel_expanded = false;
        } else {
            ctx.set_selection(Selection::SpaceView(space_view_id));
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
    spaces_info: &SpacesInfo,
    space_view: &mut SpaceView,
) {
    let Some(reference_space_info) = spaces_info.spaces.get(&space_view.reference_space_path) else {
        ui.centered_and_justified(|ui| {
            ui.label(ctx.re_ui.warning_text(format!("Unknown space {:?}", space_view.reference_space_path)));
        });
        return;
    };

    // TODO: Should be able to declare a new reference_space_info here.

    let Some(latest_at) = ctx.rec_cfg.time_ctrl.time_int() else {
        ui.centered_and_justified(|ui| {
            ui.label(ctx.re_ui.warning_text("No time selected"));
        });
        return
    };

    space_view.scene_ui(ctx, ui, spaces_info, reference_space_info, latest_at);
}

// ----------------------------------------------------------------------------

/// Defines the layout of the whole Viewer (or will, eventually).
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct Blueprint {
    pub blueprint_panel_expanded: bool,
    pub selection_panel_expanded: bool,
    pub time_panel_expanded: bool,

    pub viewport: ViewportBlueprint,
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            blueprint_panel_expanded: true,
            selection_panel_expanded: true,
            time_panel_expanded: true,
            viewport: Default::default(),
        }
    }
}

impl Blueprint {
    pub fn blueprint_panel_and_viewport(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let spaces_info = SpacesInfo::new(&ctx.log_db.obj_db, &ctx.rec_cfg.time_ctrl);

        self.viewport.on_frame_start(ctx, &spaces_info);

        self.blueprint_panel(ctx, ui, &spaces_info);

        let viewport_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(viewport_frame)
            .show_inside(ui, |ui| {
                self.viewport.viewport_ui(
                    ui,
                    ctx,
                    &spaces_info,
                    &mut self.selection_panel_expanded,
                );
            });
    }

    fn blueprint_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
    ) {
        let panel = egui::SidePanel::left("blueprint_panel")
            .resizable(true)
            .frame(ctx.re_ui.panel_frame())
            .min_width(120.0)
            .default_width(200.0);

        panel.show_animated_inside(ui, self.blueprint_panel_expanded, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.vertical_centered(|ui| {
                    ui.strong("Blueprint");
                });
            });

            ui.separator();

            ui.vertical_centered(|ui| {
                if ui
                    .button("Auto-populate Viewport")
                    .on_hover_text("Re-populate Viewport with automatically chosen Space Views")
                    .clicked()
                {
                    self.viewport = ViewportBlueprint::new(ctx, spaces_info);
                }
            });

            ui.separator();

            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::remainder())
                .size(egui_extras::Size::exact(20.0))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        self.viewport
                            .tree_ui(ctx, ui, spaces_info, &ctx.log_db.obj_db.tree);
                    });
                    strip.cell(|ui| {
                        self.viewport.create_new_blueprint_ui(ctx, ui, spaces_info);
                    });
                });
        });
    }
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
