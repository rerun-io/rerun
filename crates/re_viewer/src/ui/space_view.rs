use std::collections::BTreeMap;

use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityTree, InstanceId, TimeInt};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpaceInfoCollection},
        SpaceViewHighlights, TransformCache, UnreachableTransform, ViewerContext,
    },
    ui::view_category::categorize_entity_path,
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
    pub root_path: EntityPath,

    /// The "anchor point" of this space view.
    /// It refers to a [`SpaceInfo`] which forms our reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_path: EntityPath,

    /// The data blueprint tree, has blueprint settings for all blueprint groups and entities in this spaceview.
    /// It determines which entities are part of the spaceview.
    pub data_blueprint: DataBlueprintTree,

    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    /// Set to `false` the first time the user messes around with the list of queried entities.
    pub allow_auto_adding_more_entities: bool,
}

impl SpaceView {
    pub fn new(
        category: ViewCategory,
        space_info: &SpaceInfo,
        queries_entities: &[EntityPath],
    ) -> Self {
        let root_path = space_info.path.iter().next().map_or_else(
            || space_info.path.clone(),
            |c| EntityPath::from(vec![c.clone()]),
        );

        let name = if queries_entities.len() == 1 {
            // a single entity in this space-view - name the space after it
            queries_entities[0].to_string()
        } else {
            space_info.path.to_string()
        };

        let mut data_blueprint_tree = DataBlueprintTree::default();
        data_blueprint_tree
            .insert_entities_according_to_hierarchy(queries_entities.iter(), &space_info.path);

        Self {
            name,
            id: SpaceViewId::random(),
            root_path,
            space_path: space_info.path.clone(),
            data_blueprint: data_blueprint_tree,
            view_state: ViewState::default(),
            category,
            allow_auto_adding_more_entities: true,
        }
    }

    /// List of entities a space view queries by default for a given category.
    ///
    /// These are all entities in the given space which have the requested category and are reachable by a transform.
    pub fn default_queries_entities(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
        space_info: &SpaceInfo,
        category: ViewCategory,
    ) -> Vec<EntityPath> {
        crate::profile_function!();

        let timeline = Timeline::log_time();
        let log_db = &ctx.log_db;

        let mut entities = Vec::new();

        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            entities.extend(
                space_info
                    .descendants_without_transform
                    .iter()
                    .filter(|entity_path| {
                        categorize_entity_path(timeline, log_db, entity_path).contains(category)
                    })
                    .cloned(),
            );
        });

        entities
    }

    /// List of entities a space view queries by default for all any possible category.
    pub fn default_queries_entities_by_category(
        ctx: &ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
        space_info: &SpaceInfo,
    ) -> BTreeMap<ViewCategory, Vec<EntityPath>> {
        crate::profile_function!();

        let timeline = Timeline::log_time();
        let log_db = &ctx.log_db;

        let mut groups: BTreeMap<ViewCategory, Vec<EntityPath>> = BTreeMap::default();

        space_info.visit_descendants_with_reachable_transform(spaces_info, &mut |space_info| {
            for entity_path in &space_info.descendants_without_transform {
                for category in categorize_entity_path(timeline, log_db, entity_path) {
                    groups
                        .entry(category)
                        .or_default()
                        .push(entity_path.clone());
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

        if self.allow_auto_adding_more_entities {
            // Add entities that have been logged since we were created
            let queries_entities =
                Self::default_queries_entities(ctx, spaces_info, space_info, self.category);
            self.data_blueprint
                .insert_entities_according_to_hierarchy(queries_entities.iter(), &self.space_path);
        }
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.label("Space path:");
        // specify no space view id since the path itself is not part of the space view.
        ctx.entity_path_button(ui, None, &self.space_path);
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
        let entity_tree = &ctx.log_db.entity_db.tree;

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
            if let Some(subtree) = entity_tree.subtree(&self.root_path) {
                let spaces_info = SpaceInfoCollection::new(&ctx.log_db.entity_db);
                self.entity_tree_children_ui(ctx, ui, &spaces_info, subtree);
            }
        });
    }

    fn entity_tree_children_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        tree: &EntityTree,
    ) {
        if tree.children.is_empty() {
            ui.weak("(nothing)");
            return;
        }

        for (path_comp, child_tree) in &tree.children {
            self.entity_tree_ui(ctx, ui, spaces_info, &path_comp.to_string(), child_tree);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn entity_tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
        name: &str,
        tree: &EntityTree,
    ) {
        let unreachable_reason = spaces_info.is_reachable_by_transform(&tree.path, &self.root_path).map_err
            (|reason| match reason {
                // Should never happen
                UnreachableTransform::Unconnected =>
                     "No entity path connection from this space view.",
                UnreachableTransform::NestedPinholeCameras =>
                    "Can't display entities under nested pinhole cameras.",
                UnreachableTransform::UnknownTransform =>
                    "Can't display entities that are connected via an unknown transform to this space.",
                UnreachableTransform::InversePinholeCameraWithoutResolution =>
                    "Can't display entities that would require inverting a pinhole camera without a specified resolution.",
            }).err();
        let response = if tree.is_leaf() {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(unreachable_reason.is_none(), |ui| {
                    self.entity_path_button_ui(ctx, ui, &tree.path, spaces_info, name);
                    if has_visualization_for_category(ctx, self.category, &tree.path) {
                        self.add_entity_button_ui(ctx, ui, &tree.path, &ctx.log_db.entity_db.tree);
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
                    self.entity_path_button_ui(ctx, ui, &tree.path, spaces_info, name);
                    if has_visualization_for_category(ctx, self.category, &tree.path) {
                        self.add_entity_button_ui(ctx, ui, &tree.path, &ctx.log_db.entity_db.tree);
                    }
                });
            })
            .body(|ui| {
                self.entity_tree_children_ui(ctx, ui, spaces_info, tree);
            })
            .0
        };

        if let Some(unreachable_reason) = unreachable_reason {
            response.on_hover_text(unreachable_reason);
        }
    }

    pub fn entity_path_button_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        path: &EntityPath,
        spaces_info: &SpaceInfoCollection,
        name: &str,
    ) {
        let mut is_space_info = false;
        let label_text = if spaces_info.get(path).is_some() {
            is_space_info = true;
            let label_text = egui::RichText::new(format!("📐 {name}"));
            if *path == self.space_path {
                label_text.strong()
            } else {
                label_text
            }
        } else {
            egui::RichText::new(name)
        };

        if ctx
            .data_blueprint_button_to(ui, label_text, self.id, path)
            .double_clicked()
            && is_space_info
        {
            // TODO(andreas): Can't yet change the reference space.
            //*reference_space = path.clone();
        }
    }

    fn add_entity_button_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        path: &EntityPath,
        entity_tree: &EntityTree,
    ) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Can't add things we already added.

            // Insert the entity itself and all its children as far as they haven't been added yet
            let mut entities = Vec::new();
            entity_tree
                .subtree(path)
                .unwrap()
                .visit_children_recursively(&mut |path: &EntityPath| {
                    if has_visualization_for_category(ctx, self.category, path)
                        && !self.data_blueprint.contains_entity(path)
                    {
                        entities.push(path.clone());
                    }
                });

            ui.set_enabled(!entities.is_empty());

            let response = ui.button("➕");
            if response.clicked() {
                self.data_blueprint
                    .insert_entities_according_to_hierarchy(entities.iter(), &self.space_path);
                self.allow_auto_adding_more_entities = false;
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
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_function!();

        let query = crate::ui::scene::SceneQuery {
            entity_paths: self.data_blueprint.entity_paths(),
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            entity_props_map: self.data_blueprint.data_blueprints_projected(),
        };

        match self.category {
            ViewCategory::Text => {
                let mut scene = view_text::SceneText::default();
                scene.load(ctx, &query, &self.view_state.state_text.filters);
                self.view_state.ui_text(ctx, ui, &scene);
            }

            ViewCategory::TimeSeries => {
                let mut scene = view_time_series::SceneTimeSeries::default();
                scene.load(ctx, &query);
                self.view_state.ui_time_series(ctx, ui, &scene);
            }

            ViewCategory::BarChart => {
                let mut scene = view_bar_chart::SceneBarChart::default();
                scene.load(ctx, &query);
                self.view_state.ui_bar_chart(ctx, ui, &scene);
            }

            ViewCategory::Spatial => {
                let transforms = TransformCache::determine_transforms(
                    &ctx.log_db.entity_db,
                    &ctx.rec_cfg.time_ctrl,
                    &self.space_path,
                    self.data_blueprint.data_blueprints_projected(),
                );
                let mut scene = view_spatial::SceneSpatial::default();
                scene.load(ctx, &query, &transforms, highlights);
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
                scene.load(ctx, &query);
                self.view_state.ui_tensor(ctx, ui, &scene);
            }
        };
    }
}

fn has_visualization_for_category(
    ctx: &ViewerContext<'_>,
    category: ViewCategory,
    entity_path: &EntityPath,
) -> bool {
    let log_db = &ctx.log_db;
    categorize_entity_path(Timeline::log_time(), log_db, entity_path).contains(category)
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
        space: &EntityPath,
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
