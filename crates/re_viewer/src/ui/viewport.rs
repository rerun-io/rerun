//! The viewport panel.
//!
//! Contains all space views.
//!
//! To do:
//! * [ ] Opening up new Space Views
//! * [ ] Controlling visibility of objects inside each Space View
//! * [ ] Transforming objects between spaces

use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, TimeQuery};

use crate::misc::{space_info::*, Selection, ViewerContext};

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

type VisibilitySet = BTreeMap<SpaceViewId, bool>;

/// Describes the layout and contents of the Viewport Panel.
#[derive(Default, serde::Deserialize, serde::Serialize)]
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
}

impl ViewportBlueprint {
    /// Create a default suggested blueprint using some heuristics.
    fn new(ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) -> Self {
        crate::profile_function!();

        let mut blueprint = Self::default();

        for (path, space_info) in &spaces_info.spaces {
            let query = SceneQuery {
                obj_paths: &space_info.objects,
                timeline: *ctx.rec_cfg.time_ctrl.timeline(),
                time_query: TimeQuery::LatestAt(i64::MAX),
                obj_props: &Default::default(), // all visible
            };
            let scene = query.query(ctx);
            for category in scene.categories() {
                let space_view = SpaceView::new(&scene, category, path.clone());
                let space_view_id = SpaceViewId::random();
                blueprint.space_views.insert(space_view_id, space_view);
                blueprint.visible.insert(space_view_id, true);
            }
        }

        blueprint
    }

    pub(crate) fn get_space_view_mut(
        &mut self,
        space_view: &SpaceViewId,
    ) -> Option<&mut SpaceView> {
        self.space_views.get_mut(space_view)
    }

    fn has_space(&self, space_path: &ObjPath) -> bool {
        self.space_views
            .values()
            .any(|view| &view.space_path == space_path)
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
                    .sorted_by_key(|space_view_id| {
                        &self.space_views.get(space_view_id).unwrap().name
                    })
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

        let space_path = &space_view.space_path;
        let collapsing_header_id = ui.make_persistent_id(space_view_id);
        let default_open = true;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            match space_view.category {
                ViewCategory::TwoD => ui.label("🖼"),
                ViewCategory::ThreeD => ui.label("🔭"),
                ViewCategory::Tensor => ui.label("🇹"),
                ViewCategory::Text => ui.label("📃"),
            };

            if ctx
                .space_view_button_to(ui, space_view.name.clone(), *space_view_id)
                .clicked()
            {
                if let Some(tree) = self.trees.get_mut(&self.visible) {
                    focus_tab(tree, space_view_id);
                }
            }

            let is_space_view_visible = self.visible.entry(*space_view_id).or_insert(true);
            visibility_button(ui, true, is_space_view_visible);
        })
        .body(|ui| {
            if let Some(space_info) = spaces_info.spaces.get(space_path) {
                if let Some(tree) = obj_tree.subtree(space_path) {
                    let is_space_view_visible = self.visible.entry(*space_view_id).or_insert(true);
                    show_obj_tree_children(
                        ctx,
                        ui,
                        *is_space_view_visible,
                        &mut space_view.obj_tree_properties,
                        space_info,
                        tree,
                    );
                }
            }
        });
    }

    fn add_space_view(&mut self, space_view: SpaceView) {
        let space_view_id = SpaceViewId::random();
        self.space_views.insert(space_view_id, space_view);
        self.visible.insert(space_view_id, true);
        self.trees.clear(); // Reset them
    }

    fn on_frame_start(&mut self, ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) {
        crate::profile_function!();

        if self.space_views.is_empty() {
            *self = Self::new(ctx, spaces_info);
        } else {
            crate::profile_scope!("look for missing space views");

            // Check if the blueprint is missing a space,
            // maybe one that has been added by new data:
            for (path, space_info) in &spaces_info.spaces {
                if !self.has_space(path) {
                    let query = SceneQuery {
                        obj_paths: &space_info.objects,
                        timeline: *ctx.rec_cfg.time_ctrl.timeline(),
                        time_query: TimeQuery::LatestAt(i64::MAX),
                        obj_props: &Default::default(), // all visible
                    };
                    let scene = query.query(ctx);
                    for category in scene.categories() {
                        self.add_space_view(SpaceView::new(&scene, category, path.clone()));
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

        // Lazily create a layout tree based on which SpaceViews are currently visible:
        let tree = self.trees.entry(self.visible.clone()).or_insert_with(|| {
            super::auto_layout::tree_from_space_views(
                ui.available_size(),
                &self.visible,
                &self.space_views,
            )
        });

        let num_space_views = num_tabs(tree);
        if num_space_views == 0 {
            // nothing to show
        } else if num_space_views == 1 {
            let space_view_id = first_tab(tree).unwrap();
            let space_view = self
                .space_views
                .get_mut(&space_view_id)
                .expect("Should have been populated beforehand");

            let response = ui
                .scope(|ui| space_view_ui(ctx, ui, spaces_info, space_view))
                .response;

            let frame = ctx.design_tokens.hovering_frame(ui.style());
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

            let frame = ctx.design_tokens.hovering_frame(ui.style());
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
            dock_style.show_close_buttons = false;
            dock_style.tab_include_scrollarea = false;
            dock_style.expand_tabs = true;

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
}

fn show_obj_tree(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    parent_is_visible: bool,
    obj_tree_properties: &mut ObjectTreeProperties,
    space_info: &SpaceInfo,
    name: String,
    tree: &ObjectTree,
) {
    if tree.is_leaf() {
        ui.horizontal(|ui| {
            ctx.obj_path_button_to(ui, name, &tree.path);
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
            ctx.obj_path_button_to(ui, name, &tree.path);
            object_visibility_button(ui, parent_is_visible, obj_tree_properties, &tree.path);
        })
        .body(|ui| {
            show_obj_tree_children(
                ctx,
                ui,
                parent_is_visible,
                obj_tree_properties,
                space_info,
                tree,
            );
        });
    }
}

fn show_obj_tree_children(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    parent_is_visible: bool,
    obj_tree_properties: &mut ObjectTreeProperties,
    space_info: &SpaceInfo,
    tree: &ObjectTree,
) {
    for (path_comp, child) in &tree.children {
        if space_info.objects.contains(&child.path) {
            show_obj_tree(
                ctx,
                ui,
                parent_is_visible,
                obj_tree_properties,
                space_info,
                path_comp.to_string(),
                child,
            );
        }
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
        let frame = self.ctx.design_tokens.hovering_frame(ui.style());
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

    fn inner_margin(&self) -> egui::style::Margin {
        egui::style::Margin::same(0.0)
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
        ctx.rec_cfg.selection == Selection::SpaceView(space_view_id) && *selection_panel_expanded;
    if ui
        .selectable_label(is_selected, text)
        .on_hover_text("Space View options")
        .clicked()
    {
        if is_selected {
            ctx.rec_cfg.selection = Selection::None;
            *selection_panel_expanded = false;
        } else {
            ctx.rec_cfg.selection = Selection::SpaceView(space_view_id);
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
    let Some(space_info) = spaces_info.spaces.get(&space_view.space_path) else {
        unknown_space_label(ui, &space_view.space_path);
        return
    };
    let Some(time_query) = ctx.rec_cfg.time_ctrl.time_query() else {
        invalid_space_label(ui);
        return
    };

    space_view.scene_ui(ctx, ui, spaces_info, space_info, time_query);
}

fn unknown_space_label(ui: &mut egui::Ui, space_path: &ObjPath) {
    ui.centered(|ui| {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("Unknown space {space_path}"),
        );
    });
}

fn invalid_space_label(ui: &mut egui::Ui) {
    ui.centered(|ui| {
        ui.colored_label(ui.visuals().warn_fg_color, "No time selected");
    });
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
            fill: ui.style().visuals.window_fill(),
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
        let shortcut = crate::ui::kb_shortcuts::TOGGLE_BLUEPRINT_PANEL;

        self.blueprint_panel_expanded ^= ui.input_mut().consume_shortcut(&shortcut);

        let panel_frame = ctx.design_tokens.panel_frame(ui.ctx());

        let collapsed_panel = egui::SidePanel::left("blueprint_panel_collapsed")
            .resizable(false)
            .frame(panel_frame)
            .default_width(16.0);

        let expanded_panel = egui::SidePanel::left("blueprint_panel_expanded")
            .resizable(true)
            .frame(panel_frame)
            .min_width(120.0)
            .default_width(200.0);

        egui::SidePanel::show_animated_between_inside(
            ui,
            self.blueprint_panel_expanded,
            collapsed_panel,
            expanded_panel,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed, or animating:
                    if ui
                        .small_button("⏵")
                        .on_hover_text(format!(
                            "Expand Blueprint View ({})",
                            ui.ctx().format_shortcut(&shortcut)
                        ))
                        .clicked()
                    {
                        self.blueprint_panel_expanded = true;
                    }
                } else {
                    // Expanded:
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("⏴")
                            .on_hover_text(format!(
                                "Collapse Blueprint View ({})",
                                ui.ctx().format_shortcut(&shortcut)
                            ))
                            .clicked()
                        {
                            self.blueprint_panel_expanded = false;
                        }

                        ui.vertical_centered(|ui| {
                            ui.label("Blueprint");
                        });
                    });

                    ui.separator();

                    ui.vertical_centered(|ui| {
                        if ui.button("Reset space views").clicked() {
                            self.viewport = ViewportBlueprint::new(ctx, spaces_info);
                        }
                    });

                    ui.separator();

                    self.viewport
                        .tree_ui(ctx, ui, spaces_info, &ctx.log_db.obj_db.tree);
                }
            },
        );
    }
}

// ----------------------------------------------------------------------------

// TODO(emilk): replace with https://github.com/Adanos020/egui_dock/pull/53 when we update egui_dock
fn num_tabs(tree: &egui_dock::Tree<SpaceViewId>) -> usize {
    let mut count = 0;
    for node in tree.iter() {
        if let egui_dock::Node::Leaf { tabs, .. } = node {
            count += tabs.len();
        }
    }
    count
}

// TODO(emilk): replace with https://github.com/Adanos020/egui_dock/pull/53 when we update egui_dock
fn first_tab(tree: &egui_dock::Tree<SpaceViewId>) -> Option<SpaceViewId> {
    for node in tree.iter() {
        if let egui_dock::Node::Leaf { tabs, .. } = node {
            if let Some(first) = tabs.first() {
                return Some(*first);
            }
        }
    }
    None
}

fn focus_tab(tree: &mut egui_dock::Tree<SpaceViewId>, tab: &SpaceViewId) {
    if let Some((node_index, tab_index)) = tree.find_tab(tab) {
        tree.set_focused_node(node_index);
        tree.set_active_tab(node_index, tab_index);
    }
}
