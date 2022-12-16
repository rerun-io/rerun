use re_data_store::{InstanceId, ObjPath, ObjectTree, ObjectsProperties, TimeInt};

use nohash_hasher::{IntMap, IntSet};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpacesInfo},
        ViewerContext,
    },
    ui::view_category::categorize_obj_path,
};

use super::{
    view_bar_chart,
    view_category::ViewCategory,
    view_plot,
    view_spatial::{self, SpatialNavigationMode},
    view_tensor, view_text,
};

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

/// A view of a space.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct SpaceView {
    pub id: SpaceViewId,
    pub name: String,

    /// Everything under this root is shown in the space view.
    pub root_path: ObjPath,

    /// The "anchor point" of this space view.
    /// It refers to a [`SpaceInfo`] which forms our reference point for all scene->world transforms in this space view.
    /// I.e. the position of this object path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what objects we show in this space view.
    pub space_path: ObjPath,

    /// List of all shown objects.
    /// TODO(andreas): This is a HashSet for the time being, but in the future it might be possible to add the same object twice.
    pub queried_objects: IntSet<ObjPath>,

    pub obj_properties: ObjectsProperties,

    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    /// Set to `true` the first time the user messes around with the list of queried objects.
    pub has_been_user_edited: bool,
}

impl SpaceView {
    pub fn new(
        ctx: &ViewerContext<'_>,
        category: ViewCategory,
        space_path: ObjPath,
        space_info: &SpaceInfo,
        default_spatial_naviation_mode: SpatialNavigationMode,
    ) -> Self {
        let mut view_state = ViewState::default();

        if category == ViewCategory::Spatial {
            view_state.state_spatial.nav_mode = default_spatial_naviation_mode;
        }

        let root_path = space_path
            .iter()
            .next()
            .map_or_else(|| space_path.clone(), |c| ObjPath::from(vec![c.to_owned()]));

        Self {
            name: space_path.to_string(),
            id: SpaceViewId::random(),
            root_path,
            space_path,
            queried_objects: Self::default_queried_objects(ctx, category, space_info),
            obj_properties: Default::default(),
            view_state,
            category,
            has_been_user_edited: false,
        }
    }

    /// List of objects a space view queries by default.
    fn default_queried_objects(
        ctx: &ViewerContext<'_>,
        category: ViewCategory,
        space_info: &SpaceInfo,
    ) -> IntSet<ObjPath> {
        crate::profile_function!();

        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let log_db = &ctx.log_db;
        space_info
            .descendants_without_transform
            .iter()
            .filter(|obj_path| categorize_obj_path(timeline, log_db, obj_path).contains(category))
            .cloned()
            .collect()
    }

    pub fn on_frame_start(&mut self, ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) {
        if self.has_been_user_edited {
            return;
        }
        let Some(space) = spaces_info.spaces.get(&self.space_path) else {
            return;
        };
        self.queried_objects = Self::default_queried_objects(ctx, self.category, space);
    }

    /// All object paths that are under the root but can't be added to the space view and why.
    ///
    /// We're not storing this since the circumstances for this may change over time.
    /// (either by choosing a different reference space path or by having new paths added)
    fn unreachable_elements(&mut self, spaces_info: &SpacesInfo) -> IntMap<ObjPath, &'static str> {
        crate::profile_function!();

        let mut forced_invisible = IntMap::default();

        let Some(reference_space) = spaces_info.spaces.get(&self.space_path) else {
            return forced_invisible; // Should never happen?
        };

        // Direct children of the current reference space.
        for (path, transform) in &reference_space.child_spaces {
            match transform {
                re_log_types::Transform::Unknown => {}

                // TODO(andreas): This should be made possible!
                re_log_types::Transform::Rigid3(_) => {
                    forced_invisible.insert(
                        path.clone(),
                        "Can't display elements with a rigid transform relative to the reference path in the same spaceview yet",
                    );
                }

                // TODO(andreas): This should be made possible *iff* the reference space itself doesn't define a pinhole camera (or is there a way to deal with that?)
                re_log_types::Transform::Pinhole(_) => {
                    forced_invisible.insert(
                        path.clone(),
                        "Can't display elements with a pinhole transform relative to the reference path in the same spaceview yet",
                    );
                }
            }
        }

        forced_invisible
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        egui::Grid::new("space_view").num_columns(2).show(ui, |ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.name);
            ui.end_row();

            ui.label("Space Path:");
            ctx.obj_path_button(ui, &self.space_path);
            ui.end_row();
        });

        ui.separator();

        ui.strong("Query Tree");
        self.query_tree_ui(ctx, ui);

        ui.separator();

        match self.category {
            ViewCategory::Spatial => {
                ui.strong("Spatial view");
                self.view_state.state_spatial.show_settings_ui(ctx, ui);
            }
            ViewCategory::Tensor => {
                if let Some(selected_tensor) = &self.view_state.selected_tensor {
                    if let Some(state_tensor) =
                        self.view_state.state_tensors.get_mut(selected_tensor)
                    {
                        ui.strong("Tensor view");
                        state_tensor.ui(ctx, ui);
                    }
                }
            }
            ViewCategory::Text => {
                ui.strong("Text view");
                ui.add_space(4.0);
                self.view_state.state_text.selection_ui(ui);
            }
            ViewCategory::Plot | ViewCategory::BarChart => {}
        }
    }

    fn query_tree_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        let obj_tree = &ctx.log_db.obj_db.tree;

        // We'd like to see the reference space path by default.
        let default_open = self.root_path != self.space_path;
        let collapsing_header_id = ui.make_persistent_id(self.id);
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            ui.label(self.root_path.to_string());
        })
        .body(|ui| {
            if let Some(subtree) = obj_tree.subtree(&self.root_path) {
                let spaces_info = SpacesInfo::new(&ctx.log_db.obj_db, &ctx.rec_cfg.time_ctrl);
                let forced_invisible = self.unreachable_elements(&spaces_info);
                self.show_obj_tree_children(ctx, ui, &spaces_info, subtree, &forced_invisible);
            }
        });
    }

    fn show_obj_tree_children(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        tree: &ObjectTree,
        forced_invisible: &IntMap<ObjPath, &str>,
    ) {
        if tree.children.is_empty() {
            ui.weak("(nothing)");
            return;
        }

        for (path_comp, child_tree) in &tree.children {
            self.show_obj_tree(
                ctx,
                ui,
                spaces_info,
                &path_comp.to_string(),
                child_tree,
                forced_invisible,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_obj_tree(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        name: &str,
        tree: &ObjectTree,
        forced_invisible: &IntMap<ObjPath, &str>,
    ) {
        let disabled_reason = if self.space_path.is_descendant_of(&tree.path) {
            Some(&"Can't display entities that aren't children of the reference path yet.")
        } else {
            forced_invisible.get(&tree.path)
        };
        let response = ui
            .add_enabled_ui(disabled_reason.is_none(), |ui| {
                if tree.is_leaf() {
                    ui.horizontal(|ui| {
                        self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                        if has_visualization_for_category(ctx, self.category, &tree.path) {
                            self.object_add_button(ui, &tree.path, &ctx.log_db.obj_db.tree);
                        }
                    });
                } else {
                    let collapsing_header_id = ui.id().with(&tree.path);

                    // Default open so that the reference path is visible.
                    let default_open = self.space_path.is_descendant_of(&tree.path);
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        collapsing_header_id,
                        default_open,
                    )
                    .show_header(ui, |ui| {
                        self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                        if has_visualization_for_category(ctx, self.category, &tree.path) {
                            self.object_add_button(ui, &tree.path, &ctx.log_db.obj_db.tree);
                        }
                    })
                    .body(|ui| {
                        self.show_obj_tree_children(ctx, ui, spaces_info, tree, forced_invisible);
                    });
                }
            })
            .response;

        if let Some(disabled_reason) = disabled_reason {
            response.on_hover_text(*disabled_reason);
        }
    }

    pub fn object_path_button(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        path: &ObjPath,
        spaces_info: &SpacesInfo,
        name: &str,
    ) {
        let mut is_space_info = false;
        let label_text = if spaces_info.spaces.contains_key(path) {
            is_space_info = true;
            let label_text = egui::RichText::new(format!("📐 {}", name));
            if *path == self.space_path {
                label_text.strong()
            } else {
                label_text
            }
        } else {
            egui::RichText::new(name)
        };

        if ctx
            .space_view_obj_path_button_to(ui, label_text, self.id, path)
            .double_clicked()
            && is_space_info
        {
            // TODO(andreas): Can't yet change the reference space.
            //*reference_space = path.clone();
        }
    }

    fn object_add_button(&mut self, ui: &mut egui::Ui, path: &ObjPath, obj_tree: &ObjectTree) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Can't add things we already added.
            ui.set_enabled(!self.queried_objects.contains(path));

            let response = ui.button("➕");
            if response.clicked() {
                // Insert the object itself and all its children as far as they haven't been added yet
                obj_tree.subtree(path).unwrap().visit_children_recursively(
                    &mut |path: &ObjPath| {
                        self.queried_objects.insert(path.clone());
                    },
                );
                self.has_been_user_edited = true;
            }
            response.on_hover_text("Add to this Space View's query")
        });
    }

    pub(crate) fn scene_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        reference_space_info: &SpaceInfo,
        latest_at: TimeInt,
    ) {
        crate::profile_function!();

        let query = crate::ui::scene::SceneQuery {
            obj_paths: &self.queried_objects,
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            obj_props: &self.obj_properties,
        };

        match self.category {
            ViewCategory::BarChart => {
                let mut scene = view_bar_chart::SceneBarChart::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_bar_chart(ctx, ui, &scene);
            }

            ViewCategory::Spatial => {
                let mut scene = view_spatial::SceneSpatial::default();
                scene.load_objects(
                    ctx,
                    &query,
                    &self.obj_properties,
                    self.view_state.state_spatial.hovered_instance_hash(),
                );
                self.view_state.ui_spatial(
                    ctx,
                    ui,
                    &self.space_path,
                    spaces_info,
                    reference_space_info,
                    scene,
                );
            }

            ViewCategory::Tensor => {
                ui.add_space(16.0); // Extra headroom required for the hovering controls at the top of the space view.

                let mut scene = view_tensor::SceneTensor::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_tensor(ctx, ui, &scene);
            }

            ViewCategory::Text => {
                let mut scene = view_text::SceneText::default();
                scene.load_objects(ctx, &query, &self.view_state.state_text.filters);
                self.view_state.ui_text(ctx, ui, &scene);
            }

            ViewCategory::Plot => {
                let mut scene = view_plot::ScenePlot::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_plot(ctx, ui, &scene);
            }
        };
    }
}

fn has_visualization_for_category(
    ctx: &ViewerContext<'_>,
    category: ViewCategory,
    obj_path: &ObjPath,
) -> bool {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let log_db = &ctx.log_db;
    categorize_obj_path(timeline, log_db, obj_path).contains(category)
}

// ----------------------------------------------------------------------------

/// Show help-text on top of space
fn show_help_button_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    ctx: &mut ViewerContext<'_>,
    help_text: &str,
) {
    let mut ui = ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::TOP));
    ctx.re_ui.hovering_frame().show(&mut ui, |ui| {
        crate::misc::help_hover_button(ui).on_hover_text(help_text);
    });
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
    /// Selects in [`Self::state_tensors`].
    selected_tensor: Option<InstanceId>,

    pub state_spatial: view_spatial::ViewSpatialState,
    state_tensors: ahash::HashMap<InstanceId, view_tensor::ViewTensorState>,
    state_text: view_text::ViewTextState,
    state_plot: view_plot::ViewPlotState,
    state_bar_chart: view_bar_chart::BarChartState,
}

impl ViewState {
    fn ui_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: view_spatial::SceneSpatial,
    ) {
        ui.vertical(|ui| {
            let response =
                self.state_spatial
                    .view_spatial(ctx, ui, space, scene, spaces_info, space_info);
            show_help_button_overlay(ui, response.rect, ctx, self.state_spatial.help_text());
        });
    }

    fn ui_tensor(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_tensor::SceneTensor,
    ) {
        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            if scene.tensors.is_empty() {
                ui.centered_and_justified(|ui| ui.label("(empty)"));
                self.selected_tensor = None;
            } else {
                if let Some(selected_tensor) = &self.selected_tensor {
                    if !scene.tensors.contains_key(selected_tensor) {
                        self.selected_tensor = None;
                    }
                }
                if self.selected_tensor.is_none() {
                    self.selected_tensor = Some(scene.tensors.iter().next().unwrap().0.clone());
                }

                if scene.tensors.len() > 1 {
                    // Show radio buttons for the different tensors we have in this view - better than nothing!
                    ui.horizontal(|ui| {
                        for instance_id in scene.tensors.keys() {
                            let is_selected = self.selected_tensor.as_ref() == Some(instance_id);
                            if ui.radio(is_selected, instance_id.to_string()).clicked() {
                                self.selected_tensor = Some(instance_id.clone());
                            }
                        }
                    });
                }

                if let Some(selected_tensor) = &self.selected_tensor {
                    if let Some(tensor) = scene.tensors.get(selected_tensor) {
                        let state_tensor = self
                            .state_tensors
                            .entry(selected_tensor.clone())
                            .or_insert_with(|| view_tensor::ViewTensorState::create(tensor));
                        view_tensor::view_tensor(ctx, ui, state_tensor, tensor);
                    }
                }
            }
        });
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_text::SceneText,
    ) {
        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            view_text::view_text(ctx, ui, &mut self.state_text, scene);
        });
    }

    fn ui_bar_chart(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_bar_chart::SceneBarChart,
    ) {
        ui.vertical(|ui| {
            let response = ui
                .scope(|ui| {
                    view_bar_chart::view_bar_chart(ctx, ui, &mut self.state_bar_chart, scene);
                })
                .response;

            show_help_button_overlay(ui, response.rect, ctx, view_bar_chart::HELP_TEXT);
        });
    }

    fn ui_plot(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_plot::ScenePlot,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let response = ui
                .scope(|ui| {
                    view_plot::view_plot(ctx, ui, &mut self.state_plot, scene);
                })
                .response;

            show_help_button_overlay(ui, response.rect, ctx, view_plot::HELP_TEXT);
        })
        .response
    }
}
