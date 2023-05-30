use re_arrow_store::Timeline;
use re_data_store::{EntityPath, EntityPropertyMap, EntityTree, InstancePath, TimeInt};
use re_renderer::ScreenshotProcessor;
use re_viewer_context::{EmptySpaceViewState, SpaceViewClassName, SpaceViewId, ViewerContext};

use crate::{
    data_blueprint::DataBlueprintTree,
    space_info::SpaceInfoCollection,
    space_view_heuristics::default_queried_entities,
    space_view_highlights::SpaceViewHighlights,
    transform_cache::TransformCache,
    view_bar_chart,
    view_category::{categorize_entity_path, ViewCategory},
    view_spatial, view_tensor, view_time_series,
};

// ----------------------------------------------------------------------------

#[derive(PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)] // Not used on the web.
pub enum ScreenshotMode {
    /// The screenshot will be saved to disc and copied to the clipboard.
    SaveAndCopyToClipboard,

    /// The screenshot will be copied to the clipboard.
    CopyToClipboard,
}

/// A view of a space.
#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SpaceViewBlueprint {
    pub id: SpaceViewId,
    pub display_name: String,
    pub space_view_class: SpaceViewClassName,

    /// The "anchor point" of this space view.
    /// The transform at this path forms the reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_path: EntityPath,

    /// The data blueprint tree, has blueprint settings for all blueprint groups and entities in this spaceview.
    /// It determines which entities are part of the spaceview.
    pub data_blueprint: DataBlueprintTree,

    /// We only show data that match this category.
    /// TODO(andreas): This is obsolete and should be fully replaced by the space view type framework.
    pub category: ViewCategory,

    /// True if the user is expected to add entities themselves. False otherwise.
    pub entities_determined_by_user: bool,
}

impl SpaceViewBlueprint {
    pub fn new(
        space_view_class: SpaceViewClassName,
        category: ViewCategory,
        space_path: &EntityPath,
        queries_entities: &[EntityPath],
    ) -> Self {
        // We previously named the [`SpaceView`] after the [`EntityPath`] if there was only a single entity. However,
        // this led to somewhat confusing and inconsistent behavior. See https://github.com/rerun-io/rerun/issues/1220
        // Spaces are now always named after the final element of the space-path (or the root), independent of the
        // query entities.
        let display_name = if let Some(name) = space_path.iter().last() {
            name.to_string()
        } else {
            // Include category name in the display for root paths because they look a tad bit too short otherwise.
            format!("/ ({category})")
        };

        let mut data_blueprint_tree = DataBlueprintTree::default();
        data_blueprint_tree
            .insert_entities_according_to_hierarchy(queries_entities.iter(), space_path);

        Self {
            display_name,
            space_view_class,
            id: SpaceViewId::random(),
            space_path: space_path.clone(),
            data_blueprint: data_blueprint_tree,
            category,
            entities_determined_by_user: false,
        }
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        self.data_blueprint.on_frame_start();

        if !self.entities_determined_by_user {
            // Add entities that have been logged since we were created
            let queries_entities =
                default_queried_entities(ctx, &self.space_path, spaces_info, self.category);
            self.data_blueprint
                .insert_entities_according_to_hierarchy(queries_entities.iter(), &self.space_path);
        }

        while ScreenshotProcessor::next_readback_result(
            ctx.render_ctx,
            self.id.gpu_readback_id(),
            |data, extent, mode| self.handle_pending_screenshots(data, extent, mode),
        )
        .is_some()
        {}
    }

    fn handle_pending_screenshots(&self, data: &[u8], extent: glam::UVec2, mode: ScreenshotMode) {
        // Set to clipboard.
        #[cfg(not(target_arch = "wasm32"))]
        re_viewer_context::Clipboard::with(|clipboard| {
            clipboard.set_image([extent.x as _, extent.y as _], data);
        });
        if mode == ScreenshotMode::CopyToClipboard {
            return;
        }

        // Get next available file name.
        let safe_display_name = self
            .display_name
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "");
        let mut i = 1;
        let filename = loop {
            let filename = format!("Screenshot {safe_display_name} - {i}.png");
            if !std::path::Path::new(&filename).exists() {
                break filename;
            }
            i += 1;
        };
        let filename = std::path::Path::new(&filename);

        match image::save_buffer(filename, data, extent.x, extent.y, image::ColorType::Rgba8) {
            Ok(_) => {
                re_log::info!(
                    "Saved screenshot to {:?}.",
                    filename.canonicalize().unwrap_or(filename.to_path_buf())
                );
            }
            Err(err) => {
                re_log::error!(
                    "Failed to safe screenshot to {:?}: {}",
                    filename.canonicalize().unwrap_or(filename.to_path_buf()),
                    err
                );
            }
        }
    }

    pub fn selection_ui(
        &mut self,
        view_state: &mut SpaceViewState,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) {
        if let Ok(space_view_class) = ctx.space_view_class_registry.query(self.space_view_class) {
            crate::profile_scope!("selection_ui: ", space_view_class.name());
            space_view_class.selection_ui(ctx, ui, view_state.state.as_mut());
        } else {
            // Legacy handling

            #[allow(clippy::match_same_arms)]
            match self.category {
                ViewCategory::Text => {}
                ViewCategory::TextBox => {
                    // migrated.
                }
                ViewCategory::TimeSeries => {}
                ViewCategory::BarChart => {}
                ViewCategory::Spatial => {
                    view_state.state_spatial.selection_ui(
                        ctx,
                        ui,
                        &self.data_blueprint,
                        &self.space_path,
                        self.id,
                    );
                }
                ViewCategory::Tensor => {
                    if let Some(selected_tensor) = &view_state.selected_tensor {
                        if let Some(state_tensor) =
                            view_state.state_tensors.get_mut(selected_tensor)
                        {
                            state_tensor.ui(ctx, ui);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn scene_ui(
        &mut self,
        view_state: &mut SpaceViewState,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        latest_at: TimeInt,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_function!();

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return;
        }

        let query = re_viewer_context::SceneQuery {
            entity_paths: self.data_blueprint.entity_paths(),
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            entity_props_map: self.data_blueprint.data_blueprints_projected(),
        };

        if let Ok(space_view_class) = ctx.space_view_class_registry.query(self.space_view_class) {
            let mut scene = space_view_class.new_scene();
            {
                crate::profile_scope!("scene populate: ", space_view_class.name());
                scene.populate(ctx, &query, view_state.state.as_ref());
            }
            // TODO(andreas): Pass scene to renderer.
            // TODO(andreas): Setup re_renderer view.
            {
                crate::profile_scope!("ui: ", space_view_class.name());
                space_view_class.ui(ctx, ui, view_state.state.as_mut(), scene);
            }
        } else {
            // Legacy handling
            match self.category {
                ViewCategory::Text | ViewCategory::TextBox => {
                    // migrated.
                }

                ViewCategory::TimeSeries => {
                    let mut scene = view_time_series::SceneTimeSeries::default();
                    scene.load(ctx, &query);
                    view_state.ui_time_series(ctx, ui, &scene);
                }

                ViewCategory::BarChart => {
                    let mut scene = view_bar_chart::SceneBarChart::default();
                    scene.load(ctx, &query);
                    view_state.ui_bar_chart(ctx, ui, &scene);
                }

                ViewCategory::Spatial => {
                    let transforms = TransformCache::determine_transforms(
                        &ctx.log_db.entity_db,
                        &ctx.rec_cfg.time_ctrl,
                        &self.space_path,
                        self.data_blueprint.data_blueprints_projected(),
                    );
                    let mut scene = view_spatial::SceneSpatial::new(ctx.render_ctx);
                    scene.load(ctx, &query, &transforms, highlights);
                    view_state
                        .state_spatial
                        .update_object_property_heuristics(ctx, &mut self.data_blueprint);
                    view_state.ui_spatial(
                        ctx,
                        ui,
                        &self.space_path,
                        scene,
                        self.id,
                        highlights,
                        self.data_blueprint.data_blueprints_projected(),
                    );
                }

                ViewCategory::Tensor => {
                    let mut scene = view_tensor::SceneTensor::default();
                    scene.load(ctx, &query);
                    view_state.ui_tensor(ctx, ui, &scene);
                }
            };
        }
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
pub struct SpaceViewState {
    // TODO(andreas): Reduce this struct to just this field.
    state: Box<dyn re_viewer_context::SpaceViewState>,

    /// Selects in [`Self::state_tensors`].
    selected_tensor: Option<InstancePath>,

    state_time_series: view_time_series::ViewTimeSeriesState,
    state_bar_chart: view_bar_chart::BarChartState,
    pub state_spatial: view_spatial::ViewSpatialState,
    state_tensors: ahash::HashMap<InstancePath, view_tensor::ViewTensorState>,
}

#[allow(clippy::derivable_impls)] // Clippy claims default is derivable, but it's not.
impl Default for SpaceViewState {
    fn default() -> Self {
        Self {
            state: Box::<EmptySpaceViewState>::default(),
            selected_tensor: Default::default(),
            state_time_series: Default::default(),
            state_bar_chart: Default::default(),
            state_spatial: Default::default(),
            state_tensors: Default::default(),
        }
    }
}

impl SpaceViewState {
    // TODO(andreas): split into smaller parts, some of it shouldn't be part of the ui path and instead scene loading.
    #[allow(clippy::too_many_arguments)]
    fn ui_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &EntityPath,
        scene: view_spatial::SceneSpatial,
        space_view_id: SpaceViewId,
        highlights: &SpaceViewHighlights,
        entity_properties: &EntityPropertyMap,
    ) {
        ui.vertical(|ui| {
            self.state_spatial.view_spatial(
                ctx,
                ui,
                space,
                scene,
                space_view_id,
                highlights,
                entity_properties,
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
