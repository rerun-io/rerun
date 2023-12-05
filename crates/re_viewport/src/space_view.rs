use ahash::HashSet;
use re_data_store::{EntityPath, EntityProperties, StoreDb, TimeInt, VisibleHistory};
use re_data_store::{EntityPropertiesComponent, EntityPropertyMap};

use re_log_types::EntityPathExpr;
use re_renderer::ScreenshotProcessor;
use re_space_view::{
    DataQueryBlueprint, EntityOverrides, PropertyResolver, ScreenshotMode, SpaceViewContents,
};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::blueprint::SpaceViewComponent;
use re_viewer_context::{
    DataQueryId, DataResult, DynSpaceViewClass, PerSystemDataResults, PerSystemEntities,
    SpaceViewClass, SpaceViewClassIdentifier, SpaceViewHighlights, SpaceViewId, SpaceViewState,
    SpaceViewSystemRegistry, StoreContext, ViewerContext,
};

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(Clone)]
pub struct SpaceViewBlueprint {
    pub id: SpaceViewId,
    pub display_name: String,
    class_identifier: SpaceViewClassIdentifier,

    /// The "anchor point" of this space view.
    /// The transform at this path forms the reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_origin: EntityPath,

    /// The data queries that are part of this space view.
    pub queries: Vec<DataQueryBlueprint>,

    /// True if the user is expected to add entities themselves. False otherwise.
    pub entities_determined_by_user: bool,

    /// Auto Properties
    // TODO(jleibs): This needs to be per-query
    pub auto_properties: EntityPropertyMap,
}

/// Determine whether this `SpaceViewBlueprint` has user-edits relative to another `SpaceViewBlueprint`
impl SpaceViewBlueprint {
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            id,
            display_name,
            class_identifier,
            space_origin,
            queries,
            entities_determined_by_user,
            auto_properties: _,
        } = self;

        id != &other.id
            || display_name != &other.display_name
            || class_identifier != &other.class_identifier
            || space_origin != &other.space_origin
            || queries.iter().map(|q| q.id).collect::<HashSet<_>>()
                != other.queries.iter().map(|q| q.id).collect::<HashSet<_>>()
            || entities_determined_by_user != &other.entities_determined_by_user
    }
}

impl SpaceViewBlueprint {
    pub fn new(
        space_view_class: SpaceViewClassIdentifier,
        space_view_class_display_name: &'static str,
        space_path: &EntityPath,
        query: DataQueryBlueprint,
    ) -> Self {
        // We previously named the [`SpaceView`] after the [`EntityPath`] if there was only a single entity. However,
        // this led to somewhat confusing and inconsistent behavior. See https://github.com/rerun-io/rerun/issues/1220
        // Spaces are now always named after the final element of the space-path (or the root), independent of the
        // query entities.
        let display_name = if let Some(name) = space_path.iter().last() {
            name.to_string()
        } else {
            // Include class name in the display for root paths because they look a tad bit too short otherwise.
            format!("/ ({space_view_class_display_name})")
        };

        let id = SpaceViewId::random();

        Self {
            display_name,
            class_identifier: space_view_class,
            id,
            space_origin: space_path.clone(),
            queries: vec![query],
            entities_determined_by_user: false,
            auto_properties: Default::default(),
        }
    }

    pub fn try_from_db(path: &EntityPath, blueprint_db: &StoreDb) -> Option<Self> {
        let SpaceViewComponent {
            display_name,
            class_identifier,
            space_origin,
            entities_determined_by_user,
            contents,
        } = blueprint_db
            .store()
            .query_timeless_component::<SpaceViewComponent>(path)
            .map(|c| c.value)?;

        let id = SpaceViewId::from_entity_path(path);

        let class_identifier = class_identifier.as_str().into();

        let queries = contents
            .into_iter()
            .map(DataQueryId::from)
            .filter_map(|id| {
                DataQueryBlueprint::try_from_db(
                    &id.as_entity_path(),
                    blueprint_db,
                    class_identifier,
                )
            })
            .collect();

        Some(Self {
            id,
            display_name: display_name.to_string(),
            class_identifier,
            space_origin: space_origin.into(),
            queries,
            entities_determined_by_user,
            auto_properties: Default::default(),
        })
    }

    pub fn class_identifier(&self) -> &SpaceViewClassIdentifier {
        &self.class_identifier
    }

    pub fn class<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    ) -> &'a dyn DynSpaceViewClass {
        space_view_class_registry.get_class_or_log_error(&self.class_identifier)
    }

    pub fn class_system_registry<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    ) -> &'a SpaceViewSystemRegistry {
        space_view_class_registry.get_system_registry_or_log_error(&self.class_identifier)
    }

    pub fn on_frame_start(&mut self, ctx: &ViewerContext<'_>, view_state: &mut dyn SpaceViewState) {
        while ScreenshotProcessor::next_readback_result(
            ctx.render_ctx,
            self.id.gpu_readback_id(),
            |data, extent, mode| self.handle_pending_screenshots(data, extent, mode),
        )
        .is_some()
        {}

        let query_result = ctx.lookup_query_result(self.query_id()).clone();

        // TODO(#4377): Use PerSystemDataResults
        let mut per_system_entities = PerSystemEntities::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
                    for system in &result.view_parts {
                        per_system_entities
                            .entry(*system)
                            .or_default()
                            .insert(result.entity_path.clone());
                    }
                }
            });
        }

        self.class(ctx.space_view_class_registry).on_frame_start(
            ctx,
            view_state,
            &per_system_entities,
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
        ctx: &ViewerContext<'_>,
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

        // TODO(jleibs): Sort out borrow-checker to avoid the need to clone here
        // while still being able to pass &ViewerContext down the chain.
        let query_result = ctx.lookup_query_result(self.query_id()).clone();

        let mut per_system_data_results = PerSystemDataResults::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
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
            timeline: *ctx.rec_cfg.time_ctrl.read().timeline(),
            latest_at,
            highlights,
        };

        let root_data_result = self.root_data_result(ctx.store_context);
        let props = root_data_result
            .individual_properties
            .clone()
            .unwrap_or_default();

        ui.scope(|ui| {
            class.ui(ctx, ui, view_state, &props, system_registry, &query);
        });
    }

    #[inline]
    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    #[inline]
    pub fn query_id(&self) -> DataQueryId {
        // TODO(jleibs): Return all queries
        self.queries
            .first()
            .map_or(DataQueryId::invalid(), |q| q.id)
    }

    pub fn root_data_result(&self, ctx: &StoreContext<'_>) -> DataResult {
        let entity_path = self.entity_path();

        let individual_properties = ctx
            .blueprint
            .store()
            .query_timeless_component_quiet::<EntityPropertiesComponent>(&self.entity_path())
            .map(|result| result.value.props);

        let resolved_properties = individual_properties.clone().unwrap_or_else(|| {
            let mut props = EntityProperties::default();
            // better defaults for the time series space view
            // TODO(#4194, jleibs, ab): Per-space-view-class property defaults should be factored in
            if self.class_identifier == TimeSeriesSpaceView::IDENTIFIER {
                props.visible_history.nanos = VisibleHistory::ALL;
                props.visible_history.sequences = VisibleHistory::ALL;
            }
            props
        });

        DataResult {
            entity_path: entity_path.clone(),
            view_parts: Default::default(),
            is_group: true,
            direct_included: true,
            resolved_properties,
            individual_properties,
            override_path: entity_path,
        }
    }

    // TODO(jleibs): Get rid of mut by sending blueprint update
    pub fn add_entity_exclusion(&mut self, ctx: &ViewerContext<'_>, expr: EntityPathExpr) {
        if let Some(query) = self.queries.first() {
            query.add_entity_exclusion(ctx, expr);
        }
        self.entities_determined_by_user = true;
    }

    // TODO(jleibs): Get rid of mut by sending blueprint update
    pub fn add_entity_inclusion(&mut self, ctx: &ViewerContext<'_>, expr: EntityPathExpr) {
        if let Some(query) = self.queries.first() {
            query.add_entity_inclusion(ctx, expr);
        }
        self.entities_determined_by_user = true;
    }

    pub fn clear_entity_expression(&mut self, ctx: &ViewerContext<'_>, expr: &EntityPathExpr) {
        if let Some(query) = self.queries.first() {
            query.clear_entity_expression(ctx, expr);
        }
        self.entities_determined_by_user = true;
    }

    pub fn exclusions(&self) -> impl Iterator<Item = EntityPathExpr> + '_ {
        self.queries.iter().flat_map(|q| q.exclusions())
    }

    pub fn inclusions(&self) -> impl Iterator<Item = EntityPathExpr> + '_ {
        self.queries.iter().flat_map(|q| q.inclusions())
    }
}

impl SpaceViewBlueprint {
    fn resolve_entity_overrides_for_prefix(
        &self,
        ctx: &StoreContext<'_>,
        prefix: &EntityPath,
    ) -> EntityPropertyMap {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut prop_map = self.auto_properties.clone();

        let props_path = self.entity_path().join(prefix);
        if let Some(tree) = blueprint.entity_db().tree.subtree(&props_path) {
            tree.visit_children_recursively(&mut |path: &EntityPath| {
                if let Some(props) = blueprint
                    .store()
                    .query_timeless_component_quiet::<EntityPropertiesComponent>(path)
                {
                    let overridden_path =
                        EntityPath::from(&path.as_slice()[props_path.len()..path.len()]);
                    prop_map.update(overridden_path, props.value.props);
                }
            });
        }
        prop_map
    }
}

impl PropertyResolver for SpaceViewBlueprint {
    /// Helper function to lookup the properties for a given entity path.
    ///
    /// We start with the auto properties for the `SpaceView` as the base layer and
    /// then incrementally override from there.
    fn resolve_entity_overrides(&self, ctx: &StoreContext<'_>) -> EntityOverrides {
        EntityOverrides {
            root: self.root_data_result(ctx).resolved_properties,
            individual: self.resolve_entity_overrides_for_prefix(
                ctx,
                &SpaceViewContents::INDIVIDUAL_OVERRIDES_PREFIX.into(),
            ),
            group: self.resolve_entity_overrides_for_prefix(
                ctx,
                &SpaceViewContents::GROUP_OVERRIDES_PREFIX.into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use re_data_store::StoreDb;
    use re_log_types::{DataCell, DataRow, RowId, StoreId, TimePoint};
    use re_space_view::DataQuery as _;
    use re_types::archetypes::Points3D;
    use re_viewer_context::{EntitiesPerSystemPerClass, StoreContext};

    use super::*;

    fn save_override(props: EntityProperties, path: &EntityPath, store: &mut StoreDb) {
        let component = EntityPropertiesComponent { props };
        let row = DataRow::from_cells1_sized(
            RowId::random(),
            path.clone(),
            TimePoint::timeless(),
            1,
            DataCell::from([component]),
        )
        .unwrap();

        store.add_data_row(row).unwrap();
    }

    #[test]
    fn test_overrides() {
        let mut recording = StoreDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        let mut blueprint = StoreDb::new(StoreId::random(re_log_types::StoreKind::Blueprint));

        let points = Points3D::new(vec![[1.0, 2.0, 3.0]]);

        for path in [
            "parent".into(),
            "parent/skip/child1".into(),
            "parent/skip/child2".into(),
        ] {
            let row =
                DataRow::from_archetype(RowId::random(), TimePoint::timeless(), path, &points)
                    .unwrap();
            recording.add_data_row(row).ok();
        }

        let space_view = SpaceViewBlueprint::new(
            "3D".into(),
            "3D",
            &EntityPath::root(),
            DataQueryBlueprint::new(
                "3D".into(),
                [
                    &"parent".into(),
                    &"parent/skip/child1".into(),
                    &"parent/skip/child2".into(),
                ]
                .into_iter(),
            ),
        );

        let mut entities_per_system_per_class = EntitiesPerSystemPerClass::default();
        entities_per_system_per_class
            .entry("3D".into())
            .or_default()
            .entry("Points3D".into())
            .or_insert_with(|| {
                [
                    EntityPath::from("parent"),
                    EntityPath::from("parent/skipped/child1"),
                ]
                .into_iter()
                .collect()
            });

        let query = space_view.queries.first().unwrap();

        let resolver = query.build_resolver(space_view.id, &space_view.auto_properties);

        // No overrides set. Everybody has default values.
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let parent = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), false)
                .unwrap();
            let child1 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child1"), false)
                .unwrap();
            let child2 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child2"), false)
                .unwrap();

            for result in [parent, child1, child2] {
                assert_eq!(result.resolved_properties, EntityProperties::default(),);
            }

            // Now, override visibility on parent but not group
            let mut overrides = parent.individual_properties.clone().unwrap_or_default();
            overrides.visible = false;

            save_override(overrides, &parent.override_path, &mut blueprint);
        }

        // Parent is not visible, but children are
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let parent_group = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), true)
                .unwrap();
            let parent = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), false)
                .unwrap();
            let child1 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child1"), false)
                .unwrap();
            let child2 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child2"), false)
                .unwrap();

            assert!(!parent.resolved_properties.visible);

            for result in [child1, child2] {
                assert!(result.resolved_properties.visible);
            }

            // Override visibility on parent group
            let mut overrides = parent_group
                .individual_properties
                .clone()
                .unwrap_or_default();
            overrides.visible = false;

            save_override(overrides, &parent_group.override_path, &mut blueprint);
        }

        // Nobody is visible
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let parent = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), false)
                .unwrap();
            let child1 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child1"), false)
                .unwrap();
            let child2 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child2"), false)
                .unwrap();

            for result in [parent, child1, child2] {
                assert!(!result.resolved_properties.visible);
            }
        }

        // Override visible range on root
        {
            let root = space_view.root_data_result(&StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            });
            let mut overrides = root.individual_properties.clone().unwrap_or_default();
            overrides.visible_history.enabled = true;
            overrides.visible_history.nanos = VisibleHistory::ALL;

            save_override(overrides, &root.override_path, &mut blueprint);
        }

        // Everyone has visible history
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let parent = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), false)
                .unwrap();
            let child1 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child1"), false)
                .unwrap();
            let child2 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child2"), false)
                .unwrap();

            for result in [parent, child1, child2] {
                assert!(result.resolved_properties.visible_history.enabled);
                assert_eq!(
                    result.resolved_properties.visible_history.nanos,
                    VisibleHistory::ALL
                );
            }

            let mut overrides = child2.individual_properties.clone().unwrap_or_default();
            overrides.visible_history.enabled = true;

            save_override(overrides, &child2.override_path, &mut blueprint);
        }

        // Child2 has its own visible history
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let parent = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent"), false)
                .unwrap();
            let child1 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child1"), false)
                .unwrap();
            let child2 = query_result
                .tree
                .lookup_result_by_path_and_group(&EntityPath::from("parent/skip/child2"), false)
                .unwrap();

            for result in [parent, child1] {
                assert!(result.resolved_properties.visible_history.enabled);
                assert_eq!(
                    result.resolved_properties.visible_history.nanos,
                    VisibleHistory::ALL
                );
            }

            assert!(child2.resolved_properties.visible_history.enabled);
            assert_eq!(
                child2.resolved_properties.visible_history.nanos,
                VisibleHistory::OFF
            );
        }
    }
}
