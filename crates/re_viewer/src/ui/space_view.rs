use std::collections::BTreeMap;

use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityTree, InstancePath, TimeInt};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpaceInfoCollection},
        SpaceViewHighlights, TransformCache, ViewerContext,
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

    /// True if the user is expected to add entities themselves. False otherwise.
    pub entities_determined_by_user: bool,
}

impl SpaceView {
    pub fn new(
        category: ViewCategory,
        space_info: &SpaceInfo,
        queries_entities: &[EntityPath],
    ) -> Self {
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
            space_path: space_info.path.clone(),
            data_blueprint: data_blueprint_tree,
            view_state: ViewState::default(),
            category,
            entities_determined_by_user: false,
        }
    }

    /// How to show the name in the ui
    pub fn display_text(&self) -> egui::WidgetText {
        format!("{} {}", self.category.icon(), &self.name).into()
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

        let Some(space_info) = spaces_info.get(&self.space_path) else {
            return;
        };

        if !self.entities_determined_by_user {
            // Add entities that have been logged since we were created
            let queries_entities =
                Self::default_queries_entities(ctx, spaces_info, space_info, self.category);
            self.data_blueprint
                .insert_entities_according_to_hierarchy(queries_entities.iter(), &self.space_path);
        }
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        #[allow(clippy::match_same_arms)]
        match self.category {
            ViewCategory::Text => {
                self.view_state.state_text.selection_ui(ctx.re_ui, ui);
            }
            ViewCategory::TimeSeries => {}
            ViewCategory::BarChart => {}
            ViewCategory::Spatial => {
                self.view_state.state_spatial.selection_ui(
                    ctx,
                    ui,
                    &self.data_blueprint,
                    &self.space_path,
                    self.id,
                );
            }
            ViewCategory::Tensor => {
                if let Some(selected_tensor) = &self.view_state.selected_tensor {
                    if let Some(state_tensor) =
                        self.view_state.state_tensors.get_mut(selected_tensor)
                    {
                        state_tensor.ui(ctx, ui);
                    }
                }
            }
        }
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
                let mut scene = view_tensor::SceneTensor::default();
                scene.load(ctx, &query);
                self.view_state.ui_tensor(ctx, ui, &scene);
            }
        };
    }

    /// Removes a subtree of entities from the blueprint tree.
    ///
    /// Ignores all entities that aren't part of the blueprint.
    pub fn remove_entity_subtree(&mut self, tree: &EntityTree) {
        crate::profile_function!();

        tree.visit_children_recursively(&mut |path: &EntityPath| {
            self.data_blueprint.remove_entity(path);
            self.entities_determined_by_user = true;
        });
    }

    /// Adds a subtree of entities to the blueprint tree and creates groups as needed.
    ///
    /// Ignores all entities that can't be added or are already added.
    pub fn add_entity_subtree(
        &mut self,
        tree: &EntityTree,
        spaces_info: &SpaceInfoCollection,
        log_db: &re_data_store::LogDb,
    ) {
        crate::profile_function!();

        let mut entities = Vec::new();
        tree.visit_children_recursively(&mut |entity_path: &EntityPath| {
            let entity_categories =
                categorize_entity_path(Timeline::log_time(), log_db, entity_path);

            if entity_categories.contains(self.category)
                && !self.data_blueprint.contains_entity(entity_path)
                && spaces_info
                    .is_reachable_by_transform(entity_path, &self.space_path)
                    .is_ok()
            {
                entities.push(entity_path.clone());
            }
        });

        if !entities.is_empty() {
            self.data_blueprint
                .insert_entities_according_to_hierarchy(entities.iter(), &self.space_path);
            self.entities_determined_by_user = true;
        }
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ViewState {
    /// Selects in [`Self::state_tensors`].
    selected_tensor: Option<InstancePath>,

    state_text: view_text::ViewTextState,
    state_time_series: view_time_series::ViewTimeSeriesState,
    state_bar_chart: view_bar_chart::BarChartState,
    pub state_spatial: view_spatial::ViewSpatialState,
    state_tensors: ahash::HashMap<InstancePath, view_tensor::ViewTensorState>,
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
                    for instance_path in scene.tensors.keys() {
                        let is_selected = self.selected_tensor.as_ref() == Some(instance_path);
                        if ui.radio(is_selected, instance_path.to_string()).clicked() {
                            self.selected_tensor = Some(instance_path.clone());
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
