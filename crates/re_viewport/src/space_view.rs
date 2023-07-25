use re_data_store::{EntityPath, EntityTree, TimeInt};
use re_renderer::ScreenshotProcessor;
use re_space_view::{DataBlueprintTree, ScreenshotMode};
use re_viewer_context::{
    DynSpaceViewClass, SpaceViewClassName, SpaceViewHighlights, SpaceViewId, SpaceViewState,
    SpaceViewSystemRegistry, ViewerContext,
};

use crate::{
    space_info::SpaceInfoCollection,
    space_view_heuristics::{default_queried_entities, is_entity_processed_by_class},
};

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct SpaceViewBlueprint {
    pub id: SpaceViewId,
    pub display_name: String,
    class_name: SpaceViewClassName,

    /// The "anchor point" of this space view.
    /// The transform at this path forms the reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_origin: EntityPath,

    /// The data blueprint tree, has blueprint settings for all blueprint groups and entities in this spaceview.
    /// It determines which entities are part of the spaceview.
    pub data_blueprint: DataBlueprintTree,

    /// True if the user is expected to add entities themselves. False otherwise.
    pub entities_determined_by_user: bool,
}

/// Determine whether this `SpaceViewBlueprint` has user-edits relative to another `SpaceViewBlueprint`
impl SpaceViewBlueprint {
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            id,
            display_name,
            class_name,
            space_origin,
            data_blueprint,
            entities_determined_by_user,
        } = self;

        id != &other.id
            || display_name != &other.display_name
            || class_name != &other.class_name
            || space_origin != &other.space_origin
            || data_blueprint.has_edits(&other.data_blueprint)
            || entities_determined_by_user != &other.entities_determined_by_user
    }
}

impl SpaceViewBlueprint {
    pub fn new(
        space_view_class: SpaceViewClassName,
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
            // Include class name in the display for root paths because they look a tad bit too short otherwise.
            format!("/ ({space_view_class})")
        };

        let mut data_blueprint_tree = DataBlueprintTree::default();
        data_blueprint_tree
            .insert_entities_according_to_hierarchy(queries_entities.iter(), space_path);

        Self {
            display_name,
            class_name: space_view_class,
            id: SpaceViewId::random(),
            space_origin: space_path.clone(),
            data_blueprint: data_blueprint_tree,
            entities_determined_by_user: false,
        }
    }

    pub fn class_name(&self) -> &SpaceViewClassName {
        &self.class_name
    }

    pub fn class<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    ) -> &'a dyn DynSpaceViewClass {
        space_view_class_registry.get_class_or_log_error(&self.class_name)
    }

    pub fn class_system_registry<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    ) -> &'a SpaceViewSystemRegistry {
        space_view_class_registry.get_system_registry_or_log_error(&self.class_name)
    }

    pub fn on_frame_start(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        spaces_info: &SpaceInfoCollection,
    ) {
        if !self.entities_determined_by_user {
            // Add entities that have been logged since we were created
            let queries_entities =
                default_queried_entities(ctx, &self.class_name, &self.space_origin, spaces_info);
            self.data_blueprint.insert_entities_according_to_hierarchy(
                queries_entities.iter(),
                &self.space_origin,
            );
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

    pub(crate) fn scene_ui(
        &mut self,
        view_state: &mut dyn SpaceViewState,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        latest_at: TimeInt,
        highlights: &SpaceViewHighlights,
    ) {
        re_tracing::profile_function!();

        let is_zero_sized_viewport = ui.available_size().min_elem() <= 0.0;
        if is_zero_sized_viewport {
            return;
        }

        let class = self.class(ctx.space_view_class_registry);
        let system_registry = self.class_system_registry(ctx.space_view_class_registry);

        class.prepare_ui(
            ctx,
            view_state,
            &self.data_blueprint.entity_paths().clone(), // Clone to work around borrow checker.
            self.data_blueprint.data_blueprints_individual(),
        );

        // Propagate any changes that may have been made to blueprints right away.
        self.data_blueprint.propagate_individual_to_tree();

        let query = re_viewer_context::ViewQuery {
            space_view_id: self.id,
            space_origin: &self.space_origin,
            entity_paths: self.data_blueprint.entity_paths(),
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            entity_props_map: self.data_blueprint.data_blueprints_projected(),
            highlights,
        };

        ui.scope(|ui| {
            class.ui(ctx, ui, view_state, system_registry, &query);
        });
    }

    /// Removes a subtree of entities from the blueprint tree.
    ///
    /// Ignores all entities that aren't part of the blueprint.
    pub fn remove_entity_subtree(&mut self, tree: &EntityTree) {
        re_tracing::profile_function!();

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
        ctx: &ViewerContext<'_>,
        tree: &EntityTree,
        spaces_info: &SpaceInfoCollection,
    ) {
        re_tracing::profile_function!();

        let mut entities = Vec::new();
        tree.visit_children_recursively(&mut |entity_path: &EntityPath| {
            if is_entity_processed_by_class(ctx, &self.class_name, entity_path)
                && !self.data_blueprint.contains_entity(entity_path)
                && spaces_info
                    .is_reachable_by_transform(entity_path, &self.space_origin)
                    .is_ok()
            {
                entities.push(entity_path.clone());
            }
        });

        if !entities.is_empty() {
            self.data_blueprint
                .insert_entities_according_to_hierarchy(entities.iter(), &self.space_origin);
            self.entities_determined_by_user = true;
        }
    }
}
