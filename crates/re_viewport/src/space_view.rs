use nohash_hasher::IntMap;
use re_data_store::{EntityPath, EntityProperties, EntityTree, TimeInt, VisibleHistory};
use re_data_store::{EntityPropertiesComponent, EntityPropertyMap};
use re_renderer::ScreenshotProcessor;
use re_space_view::{DataQuery, PropertyResolver, ScreenshotMode, SpaceViewContents};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_viewer_context::{
    DataResult, DynSpaceViewClass, EntitiesPerSystem, PerSystemDataResults, SpaceViewClassName,
    SpaceViewHighlights, SpaceViewId, SpaceViewState, SpaceViewSystemRegistry, StoreContext,
    ViewerContext,
};

use crate::{
    space_info::SpaceInfoCollection,
    space_view_heuristics::{
        compute_heuristic_context_for_entities, is_entity_processed_by_class,
        reachable_entities_from_root,
    },
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
    pub contents: SpaceViewContents,

    /// True if the user is expected to add entities themselves. False otherwise.
    pub entities_determined_by_user: bool,

    /// Auto Properties
    // TODO(jleibs): This needs to be per-query
    #[serde(skip)]
    pub auto_properties: EntityPropertyMap,
}

// Default needed for deserialization when adding/changing fields.
impl Default for SpaceViewBlueprint {
    fn default() -> Self {
        let id = SpaceViewId::invalid();
        Self {
            id,
            display_name: "invalid".to_owned(),
            class_name: SpaceViewClassName::invalid(),
            space_origin: EntityPath::root(),
            contents: SpaceViewContents::new(id),
            entities_determined_by_user: Default::default(),
            auto_properties: Default::default(),
        }
    }
}

/// Determine whether this `SpaceViewBlueprint` has user-edits relative to another `SpaceViewBlueprint`
impl SpaceViewBlueprint {
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            id,
            display_name,
            class_name,
            space_origin,
            contents,
            entities_determined_by_user,
            auto_properties: _,
        } = self;

        id != &other.id
            || display_name != &other.display_name
            || class_name != &other.class_name
            || space_origin != &other.space_origin
            || contents.has_edits(&other.contents)
            || entities_determined_by_user != &other.entities_determined_by_user
    }
}

impl SpaceViewBlueprint {
    pub fn new<'a>(
        space_view_class: SpaceViewClassName,
        space_path: &EntityPath,
        queries_entities: impl Iterator<Item = &'a EntityPath>,
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

        let id = SpaceViewId::random();

        let mut contents = SpaceViewContents::new(id);
        contents.insert_entities_according_to_hierarchy(queries_entities, space_path);

        Self {
            display_name,
            class_name: space_view_class,
            id,
            space_origin: space_path.clone(),
            contents,
            entities_determined_by_user: false,
            auto_properties: Default::default(),
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
        view_state: &mut dyn SpaceViewState,
    ) {
        let empty_map = IntMap::default();

        let entities_per_system_for_class = ctx
            .entities_per_system_per_class
            .get(self.class_name())
            .unwrap_or(&empty_map);

        if !self.entities_determined_by_user {
            // Add entities that have been logged since we were created.
            let reachable_entities = reachable_entities_from_root(&self.space_origin, spaces_info);
            let queries_entities = reachable_entities.iter().filter(|ent_path| {
                entities_per_system_for_class
                    .iter()
                    .any(|(_, ents)| ents.contains(ent_path))
            });
            self.contents
                .insert_entities_according_to_hierarchy(queries_entities, &self.space_origin);
        }

        self.reset_systems_per_entity_path(entities_per_system_for_class);

        while ScreenshotProcessor::next_readback_result(
            ctx.render_ctx,
            self.id.gpu_readback_id(),
            |data, extent, mode| self.handle_pending_screenshots(data, extent, mode),
        )
        .is_some()
        {}

        self.class(ctx.space_view_class_registry).on_frame_start(
            ctx,
            view_state,
            self.contents.per_system_entities(),
            &mut self.auto_properties,
        );
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

        let data_results =
            self.contents
                .execute_query(self, ctx.store_context, ctx.entities_per_system_per_class);

        let mut per_system_data_results = PerSystemDataResults::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            data_results.visit(&mut |handle| {
                if let Some(result) = data_results.lookup(handle) {
                    for system in &result.view_parts {
                        per_system_data_results
                            .entry(*system)
                            .or_default()
                            .push(result);
                    }
                }
            });
        }

        let system_registry = self.class_system_registry(ctx.space_view_class_registry);
        let query = re_viewer_context::ViewQuery {
            space_view_id: self.id,
            space_origin: &self.space_origin,
            per_system_data_results: &per_system_data_results,
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
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
            self.contents.remove_entity(path);
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

        let heuristic_context = compute_heuristic_context_for_entities(ctx.store_db);

        let mut entities = Vec::new();
        tree.visit_children_recursively(&mut |entity_path: &EntityPath| {
            if is_entity_processed_by_class(
                ctx,
                &self.class_name,
                entity_path,
                heuristic_context
                    .get(entity_path)
                    .copied()
                    .unwrap_or_default(),
                &ctx.current_query(),
            ) && !self.contents.contains_entity(entity_path)
                && spaces_info
                    .is_reachable_by_transform(entity_path, &self.space_origin)
                    .is_ok()
            {
                entities.push(entity_path.clone());
            }
        });

        if !entities.is_empty() {
            self.contents
                .insert_entities_according_to_hierarchy(entities.iter(), &self.space_origin);
            self.entities_determined_by_user = true;
        }
    }

    /// Resets the [`SpaceViewContents::per_system_entities`] for all paths that are part of this space view.
    pub fn reset_systems_per_entity_path(
        &mut self,
        entities_per_system_for_class: &EntitiesPerSystem,
    ) {
        re_tracing::profile_function!();

        // TODO(andreas): We believe this is *correct* but not necessarily optimal. Pay attention
        // to the algorithmic complexity here as we consider changing the indexing and
        // access patterns of these structures in the future.
        let mut per_system_entities = re_viewer_context::PerSystemEntities::new();
        for (system, entities) in entities_per_system_for_class {
            per_system_entities.insert(
                *system,
                self.contents
                    .entity_paths()
                    .filter(|ent_path| entities.contains(ent_path))
                    .cloned()
                    .collect(),
            );
        }

        *self.contents.per_system_entities_mut() = per_system_entities;
    }

    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    pub fn root_data_result(&self, ctx: &StoreContext<'_>) -> DataResult {
        let entity_path = self.entity_path();

        let individual_properties = ctx
            .blueprint
            .store()
            .query_timeless_component::<EntityPropertiesComponent>(&self.entity_path())
            .map(|result| result.value.props);

        let resolved_properties = individual_properties.clone().unwrap_or_else(|| {
            let mut props = EntityProperties::default();
            // better defaults for the time series space view
            // TODO(#4194, jleibs, ab): Per-space-view-class property defaults should be factored in
            if self.class_name == TimeSeriesSpaceView::NAME {
                props.visible_history.nanos = VisibleHistory::ALL;
                props.visible_history.sequences = VisibleHistory::ALL;
            }
            props
        });

        DataResult {
            entity_path: entity_path.clone(),
            view_parts: Default::default(),
            is_group: true,
            resolved_properties,
            individual_properties,
            override_path: entity_path,
        }
    }
}

impl PropertyResolver for SpaceViewBlueprint {
    /// Helper function to lookup the properties for a given entity path.
    ///
    /// We start with the auto properties for the `SpaceView` as the base layer and
    /// then incrementally override from there.
    fn resolve_entity_overrides(&self, ctx: &StoreContext<'_>) -> EntityPropertyMap {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut prop_map = self.auto_properties.clone();

        let props_path = self
            .entity_path()
            .join(&SpaceViewContents::PROPERTIES_PREFIX.into());
        if let Some(tree) = blueprint.entity_db().tree.subtree(&props_path) {
            tree.visit_children_recursively(&mut |path: &EntityPath| {
                if let Some(props) = blueprint
                    .store()
                    .query_timeless_component::<EntityPropertiesComponent>(path)
                {
                    let overridden_path =
                        EntityPath::from(&path.as_slice()[props_path.len()..path.len()]);
                    prop_map.update(overridden_path, props.value.props);
                }
            });
        }
        prop_map
    }

    fn resolve_root_override(&self, ctx: &StoreContext<'_>) -> EntityProperties {
        self.root_data_result(ctx).resolved_properties
    }
}
