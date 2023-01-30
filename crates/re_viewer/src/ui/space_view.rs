use std::collections::BTreeMap;

use re_arrow_store::Timeline;
use re_data_store::{InstanceId, ObjPath, ObjectTree, TimeInt};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpaceInfoCollection},
        SpaceViewHighlights, TransformCache, UnreachableTransform, ViewerContext,
    },
    ui::view_category::categorize_obj_path,
};

use super::{
    data_blueprint::DataBlueprintTree,
    view_bar_chart,
    view_category::ViewCategory,
    view_spatial::{self},
    view_tensor, view_text, view_time_series,
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
pub struct SpaceView {
    pub id: SpaceViewId,
    pub name: String,

    /// Everything under this root *can* be shown in the space view.
    pub root_path: ObjPath,

    /// The "anchor point" of this space view.
    /// It refers to a [`SpaceInfo`] which forms our reference point for all scene->world transforms in this space view.
    /// I.e. the position of this object path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what objects we show in this space view.
    pub space_path: ObjPath,

    /// The data blueprint tree, has blueprint settings for all blueprint groups and objects in this spaceview.
    /// It determines which objects are part of the spaceview.
    pub data_blueprint: DataBlueprintTree,

    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    /// Set to `false` the first time the user messes around with the list of queried objects.
    pub allow_auto_adding_more_object: bool,
}

impl SpaceView {
    pub fn new(
        category: ViewCategory,
        space_info: &SpaceInfo,
        queried_objects: &[ObjPath],
    ) -> Self {
        let root_path = space_info.path.iter().next().map_or_else(
            || space_info.path.clone(),
            |c| ObjPath::from(vec![c.to_owned()]),
        );

        let name = if queried_objects.len() == 1 {
            // a single object in this space-view - name the space after it
            queried_objects[0].to_string()
        } else {
            space_info.path.to_string()
        };

        let mut data_blueprint_tree = DataBlueprintTree::default();
        data_blueprint_tree
            .insert_objects_according_to_hierarchy(queried_objects.iter(), &space_info.path);

        Self {
            name,
            id: SpaceViewId::random(),
            root_path,
            space_path: space_info.path.clone(),
            data_blueprint: data_blueprint_tree,
            view_state: ViewState::default(),
            category,
            allow_auto_adding_more_object: true,
        }
    }

    /// List of objects a space view queries by default for a given category.
    ///
    /// These are all objects in the given space which have the requested category and are reachable by a transform.
    pub fn default_queried_objects(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
        space_info: &SpaceInfo,
        category: ViewCategory,
    ) -> Vec<ObjPath> {
        crate::profile_function!();

        let timeline = Timeline::log_time();
        let log_db = &ctx.log_db;

        let mut objects = Vec::new();

        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            objects.extend(
                space_info
                    .descendants_without_transform
                    .iter()
                    .filter(|obj_path| {
                        categorize_obj_path(timeline, log_db, obj_path).contains(category)
                    })
                    .cloned(),
            );
        });

        objects
    }

    /// List of objects a space view queries by default for all any possible category.
    pub fn default_queried_objects_by_category(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
        space_info: &SpaceInfo,
    ) -> BTreeMap<ViewCategory, Vec<ObjPath>> {
        crate::profile_function!();

        let timeline = Timeline::log_time();
        let log_db = &ctx.log_db;

        let mut groups: BTreeMap<ViewCategory, Vec<ObjPath>> = BTreeMap::default();

        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            for obj_path in &space_info.descendants_without_transform {
                for category in categorize_obj_path(timeline, log_db, obj_path) {
                    groups.entry(category).or_default().push(obj_path.clone());
                }
            }
        });

        groups
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        self.data_blueprint.on_frame_start();

        let Some(space_info) =  spaces_info.get(&self.space_path) else {
            return;
        };

        if self.allow_auto_adding_more_object {
            // Add objects that have been logged since we were created
            let queried_objects =
                Self::default_queried_objects(ctx, spaces_info, space_info, self.category);
            self.data_blueprint
                .insert_objects_according_to_hierarchy(queried_objects.iter(), &self.space_path);
        }
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Space path:").on_hover_text(
                "Root path of this space view. All transformations are relative this.",
            );
            // specify no space view id since the path itself is not part of the space view.
            ctx.obj_path_button(ui, None, &self.space_path);
        });

        ui.separator();

        ui.strong("Add/remove data");
        ui.add_space(2.0);
        self.add_objects_ui(ctx, ui);

        ui.separator();

        match self.category {
            ViewCategory::Text => {
                ui.strong("Text view");
                ui.add_space(4.0);
                self.view_state.state_text.selection_ui(ui);
            }

            ViewCategory::TimeSeries => {
                ui.strong("Time series view");
            }

            ViewCategory::BarChart => {
                ui.strong("Bar chart view");
            }

            ViewCategory::Spatial => {
                ui.strong("Spatial view");
                self.view_state.state_spatial.settings_ui(ctx, ui);
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
        }
    }

    fn add_objects_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        // We'd like to see the reference space path by default.

        let spaces_info = SpaceInfoCollection::new(&ctx.log_db.obj_db);
        let obj_tree = &ctx.log_db.obj_db.tree;

        // All objects at the space path and below.
        if let Some(tree) = obj_tree.subtree(&self.space_path) {
            self.add_objects_tree_ui(ctx, ui, &spaces_info, &tree.path.to_string(), tree, true);
        }

        // All objects above
        let mut num_steps_up = 0; // I.e. the number of inverse transforms we had to do!
        let mut previous_path = self.space_path.clone();
        while let Some(parent) = previous_path.parent() {
            // Don't allow breaking out of the root
            if parent.is_root() {
                break;
            }

            num_steps_up += 1;
            if let Some(tree) = obj_tree.subtree(&parent) {
                for (path_comp, child_tree) in &tree.children {
                    if child_tree.path != self.space_path {
                        self.add_objects_tree_ui(
                            ctx,
                            ui,
                            &spaces_info,
                            &format!("{}{}", "../".repeat(num_steps_up), path_comp),
                            child_tree,
                            false,
                        );
                    }
                }
            }

            previous_path = parent;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_objects_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        name: &str,
        tree: &ObjectTree,
        default_open: bool,
    ) {
        if tree.is_leaf() {
            self.add_object_line_ui(ctx, ui, spaces_info, &format!("🔹 {}", name), &tree.path);
        } else {
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(name),
                default_open && tree.children.len() <= 3,
            )
            .show_header(ui, |ui| {
                self.add_object_line_ui(ctx, ui, spaces_info, name, &tree.path);
            })
            .body(|ui| {
                for (path_comp, child_tree) in &tree.children {
                    self.add_objects_tree_ui(
                        ctx,
                        ui,
                        spaces_info,
                        &path_comp.to_string(),
                        child_tree,
                        default_open,
                    );
                }
            });
        };
    }

    fn add_object_line_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        name: &str,
        ent_path: &ObjPath,
    ) {
        ui.horizontal(|ui| {
            let space_view_id = if self.data_blueprint.contains_object(ent_path) {
                Some(self.id)
            } else {
                None
            };

            let widget_text = if ent_path == &self.space_path {
                egui::RichText::new(name).strong()
            } else {
                egui::RichText::new(name)
            };
            ctx.instance_id_button_to(ui, space_view_id, &InstanceId::new(ent_path.clone(), None), widget_text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let obj_tree = &ctx.log_db.obj_db.tree;

                if self.data_blueprint.contains_object(ent_path) {
                    if ui
                    .button("➖")
                    .on_hover_text("Remove this path from the Space View")
                    .clicked()
                    {
                        // Remove all objects at and under this path
                        obj_tree.subtree(ent_path)
                        .unwrap()
                        .visit_children_recursively(&mut |path: &ObjPath| {
                            self.data_blueprint.remove_object(path);
                        });
                        self.allow_auto_adding_more_object = false;
                    }
                } else {
                    let object_category = categorize_obj_path(Timeline::log_time(), ctx.log_db, ent_path);
                    let cannot_add_reason = if object_category.contains(self.category) {
                        spaces_info.is_reachable_by_transform(ent_path, &self.space_path).map_err
                        (|reason| match reason {
                            // Should never happen
                            UnreachableTransform::Unconnected =>
                                 "No object path connection from this space view.",
                            UnreachableTransform::NestedPinholeCameras =>
                                "Can't display objects under nested pinhole cameras.",
                            UnreachableTransform::UnknownTransform =>
                                "Can't display objects that are connected via an unknown transform to this space.",
                            UnreachableTransform::InversePinholeCameraWithoutResolution =>
                                "Can't display objects that would require inverting a pinhole camera without a specified resolution.",
                        }).err()
                    } else if object_category.is_empty() {
                        Some("Object does not contain any components")
                    } else {
                        Some("Object category can't be displayed by this type of spatial view")
                    };

                    let response = ui.add_enabled_ui(cannot_add_reason.is_none(), |ui| {
                        let response = ui.button("➕");
                        if response.clicked() {
                            // Insert the object itself and all its children as far as they haven't been added yet
                            let mut objects = Vec::new();
                            obj_tree
                                .subtree(ent_path)
                                .unwrap()
                                .visit_children_recursively(&mut |path: &ObjPath| {
                                    if has_visualization_for_category(ctx, self.category, path)
                                        && !self.data_blueprint.contains_object(path)
                                        && spaces_info.is_reachable_by_transform(path, &self.space_path).is_ok()
                                    {
                                        objects.push(path.clone());
                                    }
                                });
                            self.data_blueprint.insert_objects_according_to_hierarchy(
                                objects.iter(),
                                &self.space_path,
                            );
                            self.allow_auto_adding_more_object = false;
                        }
                        response.on_hover_text("Add this path to the Space View");
                    }).response;

                    if let Some(cannot_add_reason) = cannot_add_reason {
                        response.on_hover_text(cannot_add_reason);
                    }
                }
            });
        });
    }

    pub(crate) fn scene_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        reference_space_info: &SpaceInfo,
        latest_at: TimeInt,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_function!();

        let query = crate::ui::scene::SceneQuery {
            obj_paths: self.data_blueprint.object_paths(),
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            obj_props: self.data_blueprint.data_blueprints_projected(),
        };

        match self.category {
            ViewCategory::Text => {
                let mut scene = view_text::SceneText::default();
                scene.load_objects(ctx, &query, &self.view_state.state_text.filters);
                self.view_state.ui_text(ctx, ui, &scene);
            }

            ViewCategory::TimeSeries => {
                let mut scene = view_time_series::SceneTimeSeries::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_time_series(ctx, ui, &scene);
            }

            ViewCategory::BarChart => {
                let mut scene = view_bar_chart::SceneBarChart::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_bar_chart(ctx, ui, &scene);
            }

            ViewCategory::Spatial => {
                let transforms = TransformCache::determine_transforms(
                    &ctx.log_db.obj_db,
                    &ctx.rec_cfg.time_ctrl,
                    &self.space_path,
                    self.data_blueprint.data_blueprints_projected(),
                );
                let mut scene = view_spatial::SceneSpatial::default();
                scene.load_objects(ctx, &query, &transforms, highlights);
                self.view_state.ui_spatial(
                    ctx,
                    ui,
                    &self.space_path,
                    reference_space_info,
                    scene,
                    self.id,
                    highlights,
                );
            }

            ViewCategory::Tensor => {
                ui.add_space(16.0); // Extra headroom required for the hovering controls at the top of the space view.

                let mut scene = view_tensor::SceneTensor::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_tensor(ctx, ui, &scene);
            }
        };
    }
}

fn has_visualization_for_category(
    ctx: &ViewerContext<'_>,
    category: ViewCategory,
    obj_path: &ObjPath,
) -> bool {
    let log_db = &ctx.log_db;
    categorize_obj_path(Timeline::log_time(), log_db, obj_path).contains(category)
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ViewState {
    /// Selects in [`Self::state_tensors`].
    selected_tensor: Option<InstanceId>,

    state_text: view_text::ViewTextState,
    state_time_series: view_time_series::ViewTimeSeriesState,
    state_bar_chart: view_bar_chart::BarChartState,
    pub state_spatial: view_spatial::ViewSpatialState,
    state_tensors: ahash::HashMap<InstanceId, view_tensor::ViewTensorState>,
}

impl ViewState {
    // TODO(andreas): split into smaller parts, some of it shouldn't be part of the ui path and instead scene loading.
    #[allow(clippy::too_many_arguments)]
    fn ui_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        space_info: &SpaceInfo,
        scene: view_spatial::SceneSpatial,
        space_view_id: SpaceViewId,
        highlights: &SpaceViewHighlights,
    ) {
        ui.vertical(|ui| {
            self.state_spatial.view_spatial(
                ctx,
                ui,
                space,
                scene,
                space_info,
                space_view_id,
                highlights,
            );
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
            ui.scope(|ui| {
                view_bar_chart::view_bar_chart(ctx, ui, &mut self.state_bar_chart, scene);
            });
        });
    }

    fn ui_time_series(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_time_series::SceneTimeSeries,
    ) {
        ui.vertical(|ui| {
            ui.scope(|ui| {
                view_time_series::view_time_series(ctx, ui, &mut self.state_time_series, scene);
            });
        });
    }
}
