use std::collections::BTreeMap;

use re_data_store::{InstanceId, ObjPath, ObjectTree, TimeInt};

use nohash_hasher::IntSet;

use crate::{
    misc::{
        space_info::{SpaceInfo, SpacesInfo},
        ViewerContext,
    },
    ui::transform_cache::TransformCache,
    ui::view_category::categorize_obj_path,
};

use super::{
    data_blueprint::DataBlueprintTree,
    transform_cache::{ReferenceFromObjTransform, UnreachableTransformReason},
    view_bar_chart,
    view_category::ViewCategory,
    view_spatial::{self, SpatialNavigationMode},
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

    /// The data blueprint tree, has blueprint settings for all blueprint groups and objects in this spaceview.
    /// It determines which objects are part of the spaceview.
    pub data_blueprint: DataBlueprintTree,

    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    /// Set to `false` the first time the user messes around with the list of queried objects.
    pub allow_auto_adding_more_object: bool,

    /// Transforms seen last frame, renewed every frame.
    /// TODO(andreas): This should probably live on `SpacesInfo` and created there lazily?
    ///                 See also [#741](https://github.com/rerun-io/rerun/issues/741)
    #[serde(skip)]
    cached_transforms: TransformCache,
}

impl SpaceView {
    pub fn new(
        category: ViewCategory,
        space_info: &SpaceInfo,
        queried_objects: &IntSet<ObjPath>,
        default_spatial_navigation_mode: SpatialNavigationMode,
        initial_transforms: TransformCache,
    ) -> Self {
        let mut view_state = ViewState::default();

        if category == ViewCategory::Spatial {
            view_state.state_spatial.nav_mode = default_spatial_navigation_mode;
        }

        let root_path = space_info.path.iter().next().map_or_else(
            || space_info.path.clone(),
            |c| ObjPath::from(vec![c.to_owned()]),
        );

        let name = if queried_objects.len() == 1 {
            // a single object in this space-view - name the space after it
            let obj_path = queried_objects.iter().next().unwrap();
            obj_path.to_string()
        } else {
            space_info.path.to_string()
        };

        let mut data_blueprint_tree = DataBlueprintTree::default();
        data_blueprint_tree
            .insert_objects_according_to_hierarchy(queried_objects, &space_info.path);

        Self {
            name,
            id: SpaceViewId::random(),
            root_path,
            space_path: space_info.path.clone(),
            data_blueprint: data_blueprint_tree,
            view_state,
            category,
            allow_auto_adding_more_object: true,
            cached_transforms: initial_transforms,
        }
    }

    /// List of objects a space view queries by default for a given category.
    ///
    /// These are all objects in the given space which have the requested category and are reachable by a transform.
    pub fn default_queried_objects(
        ctx: &ViewerContext<'_>,
        category: ViewCategory,
        space_info: &SpaceInfo,
        spaces_info: &SpacesInfo,
        transforms: &TransformCache,
    ) -> IntSet<ObjPath> {
        crate::profile_function!();

        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let log_db = &ctx.log_db;

        let mut objects = IntSet::default();
        space_info.visit_descendants(spaces_info, &mut |space| {
            objects.extend(
                space
                    .descendants_without_transform
                    .iter()
                    .filter(|obj_path| {
                        transforms.reference_from_obj(obj_path).is_reachable()
                            && categorize_obj_path(timeline, log_db, obj_path).contains(category)
                    })
                    .cloned(),
            );
        });
        objects
    }

    /// List of objects a space view queries by default for all any possible category.
    pub fn default_queried_objects_by_category(
        ctx: &ViewerContext<'_>,
        space_info: &SpaceInfo,
        transforms: &TransformCache,
    ) -> BTreeMap<ViewCategory, IntSet<ObjPath>> {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        let log_db = &ctx.log_db;

        let mut groups: BTreeMap<ViewCategory, IntSet<ObjPath>> = Default::default();
        for obj_path in transforms.objects_with_reachable_transform() {
            if obj_path == &space_info.path || obj_path.is_descendant_of(&space_info.path) {
                for category in categorize_obj_path(timeline, log_db, obj_path) {
                    groups.entry(category).or_default().insert(obj_path.clone());
                }
            }
        }
        groups
    }

    pub fn on_frame_start(&mut self, ctx: &mut ViewerContext<'_>, spaces_info: &SpacesInfo) {
        self.data_blueprint.on_frame_start();

        let Some(space_info) =  spaces_info.get(&self.space_path) else {
            return;
        };

        self.cached_transforms = TransformCache::determine_transforms(
            spaces_info,
            space_info,
            self.data_blueprint.data_blueprints_projected(),
        );

        if self.allow_auto_adding_more_object {
            // Add objects that have been logged since we were created
            let queried_objects = Self::default_queried_objects(
                ctx,
                self.category,
                space_info,
                spaces_info,
                &self.cached_transforms,
            );
            self.data_blueprint
                .insert_objects_according_to_hierarchy(&queried_objects, &self.space_path);
        }
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.label("Space path:");
        ctx.obj_path_button(ui, &self.space_path);
        ui.end_row();

        ui.separator();

        ui.strong("Query tree");
        self.query_tree_ui(ctx, ui);

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
                if let Some(reference_space) = spaces_info.get(&self.space_path) {
                    let transforms = TransformCache::determine_transforms(
                        &spaces_info,
                        reference_space,
                        self.data_blueprint.data_blueprints_projected(),
                    );
                    self.obj_tree_children_ui(ctx, ui, &spaces_info, subtree, &transforms);
                }
            }
        });
    }

    fn obj_tree_children_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        tree: &ObjectTree,
        transforms: &TransformCache,
    ) {
        if tree.children.is_empty() {
            ui.weak("(nothing)");
            return;
        }

        for (path_comp, child_tree) in &tree.children {
            self.obj_tree_ui(
                ctx,
                ui,
                spaces_info,
                &path_comp.to_string(),
                child_tree,
                transforms,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn obj_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        name: &str,
        tree: &ObjectTree,
        transforms: &TransformCache,
    ) {
        let unreachable_reason = match transforms.reference_from_obj(&tree.path) {
            ReferenceFromObjTransform::Unreachable(reason) => Some(match reason {
                // Should never happen
                UnreachableTransformReason::Unconnected =>
                     "No object path connection from this space view.",
                UnreachableTransformReason::NestedPinholeCameras =>
                    "Can't display objects under nested pinhole cameras.",
                UnreachableTransformReason::UnknownTransform =>
                    "Can't display objects that are connected via an unknown transform to this space.",
                UnreachableTransformReason::InversePinholeCameraWithoutResolution =>
                    "Can't display objects that would require inverting a pinhole camera without a specified resolution.",
                }),
            ReferenceFromObjTransform::Reachable(_) => None,
        };
        let response = if tree.is_leaf() {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(unreachable_reason.is_none(), |ui| {
                    self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                    if has_visualization_for_category(ctx, self.category, &tree.path) {
                        self.object_add_button(ctx, ui, &tree.path, &ctx.log_db.obj_db.tree);
                    }
                });
            })
            .response
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
                ui.add_enabled_ui(unreachable_reason.is_none(), |ui| {
                    self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                    if has_visualization_for_category(ctx, self.category, &tree.path) {
                        self.object_add_button(ctx, ui, &tree.path, &ctx.log_db.obj_db.tree);
                    }
                });
            })
            .body(|ui| {
                self.obj_tree_children_ui(ctx, ui, spaces_info, tree, transforms);
            })
            .0
        };

        if let Some(unreachable_reason) = unreachable_reason {
            response.on_hover_text(unreachable_reason);
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
        let label_text = if spaces_info.get(path).is_some() {
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

    fn object_add_button(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        path: &ObjPath,
        obj_tree: &ObjectTree,
    ) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Can't add things we already added.

            // Insert the object itself and all its children as far as they haven't been added yet
            let mut objects = IntSet::default();
            obj_tree
                .subtree(path)
                .unwrap()
                .visit_children_recursively(&mut |path: &ObjPath| {
                    if has_visualization_for_category(ctx, self.category, path)
                        && !self.data_blueprint.contains_object(path)
                    {
                        objects.insert(path.clone());
                    }
                });

            ui.set_enabled(!objects.is_empty());

            let response = ui.button("➕");
            if response.clicked() {
                self.data_blueprint
                    .insert_objects_according_to_hierarchy(&objects, &self.space_path);
                self.allow_auto_adding_more_object = false;
            }
            response.on_hover_text("Add to this Space View's query")
        });
    }

    pub(crate) fn scene_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        reference_space_info: &SpaceInfo,
        latest_at: TimeInt,
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
                let mut scene = view_spatial::SceneSpatial::default();
                scene.load_objects(
                    ctx,
                    &query,
                    &self.cached_transforms,
                    self.view_state.state_spatial.hovered_instance_hash(),
                );
                self.view_state
                    .ui_spatial(ctx, ui, &self.space_path, reference_space_info, scene);
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
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let log_db = &ctx.log_db;
    categorize_obj_path(timeline, log_db, obj_path).contains(category)
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
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
    fn ui_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        space_info: &SpaceInfo,
        scene: view_spatial::SceneSpatial,
    ) {
        ui.vertical(|ui| {
            self.state_spatial
                .view_spatial(ctx, ui, space, scene, space_info);
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
