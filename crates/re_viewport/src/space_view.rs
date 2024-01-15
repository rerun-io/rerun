use itertools::{FoldWhile, Itertools};
use re_data_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath, EntityProperties, TimeInt, VisibleHistory};
use re_entity_db::{EntityPropertiesComponent, EntityPropertyMap};

use re_log_types::{DataRow, EntityPathFilter, EntityPathRule, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_renderer::ScreenshotProcessor;
use re_space_view::{DataQueryBlueprint, ScreenshotMode};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::blueprint::components::{EntitiesDeterminedByUser, Name, SpaceViewOrigin, Visible};
use re_types_core::archetypes::Clear;
use re_viewer_context::{
    DataQueryId, DataResult, DynSpaceViewClass, PerSystemDataResults, PerSystemEntities,
    PropertyOverrides, SpaceViewClass, SpaceViewClassIdentifier, SpaceViewHighlights, SpaceViewId,
    SpaceViewState, StoreContext, SystemCommand, SystemCommandSender as _, SystemExecutionOutput,
    ViewQuery, ViewerContext,
};

use crate::system_execution::create_and_run_space_view_systems;

// ----------------------------------------------------------------------------

/// A view of a space.
///
/// Note: [`SpaceViewBlueprint`] doesn't implement Clone because it stores an internal
/// uuid used for identifying the path of its data in the blueprint store. It's ambiguous
/// whether the intent is for a clone to write to the same place.
///
/// If you want a new space view otherwise identical to an existing one, use
/// [`SpaceViewBlueprint::duplicate`].
pub struct SpaceViewBlueprint {
    pub id: SpaceViewId,
    pub display_name: Option<String>,
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

    /// True if this space view is visible in the UI.
    pub visible: bool,
}

impl SpaceViewBlueprint {
    /// Creates a new [`SpaceViewBlueprint`] with a single [`DataQueryBlueprint`].
    ///
    /// This [`SpaceViewBlueprint`] is ephemeral. If you want to make it permanent you
    /// must call [`Self::save_to_blueprint_store`].
    pub fn new(
        space_view_class: SpaceViewClassIdentifier,
        space_path: &EntityPath,
        query: DataQueryBlueprint,
    ) -> Self {
        let id = SpaceViewId::random();

        Self {
            display_name: None,
            class_identifier: space_view_class,
            id,
            space_origin: space_path.clone(),
            queries: vec![query],
            entities_determined_by_user: false,
            visible: true,
        }
    }

    /// Placeholder name displayed in the UI if the user hasn't explicitly named the space view.
    #[allow(clippy::unused_self)]
    pub fn missing_name_placeholder(&self) -> String {
        let entity_path = self
            .space_origin
            .as_slice()
            .iter()
            .rev()
            .fold_while(String::new(), |acc, path| {
                if acc.len() > 10 {
                    FoldWhile::Done(format!("…/{acc}"))
                } else {
                    FoldWhile::Continue(format!(
                        "{}{}{}",
                        path.ui_string(),
                        if acc.is_empty() { "" } else { "/" },
                        acc
                    ))
                }
            })
            .into_inner();

        if entity_path.is_empty() {
            "/".to_owned()
        } else {
            entity_path
        }
    }

    /// Returns this space view's display name, along with a flag indicating whether it has actually been set by the
    /// user or not.
    ///
    /// When the flag is `false`, the UI should display the resulting name in italics and with a gamma of 0.5 over the
    /// text color.
    pub fn display_name_or_default(&self) -> (String, bool) {
        self.display_name.clone().map_or_else(
            || (self.missing_name_placeholder(), false),
            |name| (name, true),
        )
    }

    /// Attempt to load a [`SpaceViewBlueprint`] from the blueprint store.
    pub fn try_from_db(id: SpaceViewId, blueprint_db: &EntityDb) -> Option<Self> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());

        let re_types::blueprint::archetypes::SpaceViewBlueprint {
            display_name,
            class_identifier,
            space_origin,
            entities_determined_by_user,
            contents,
            visible,
        } = query_archetype(blueprint_db.store(), &query, &id.as_entity_path())
            .and_then(|arch| arch.to_archetype())
            .map_err(|err| {
                if !matches!(err, re_query::QueryError::PrimaryNotFound(_)) {
                    if cfg!(debug_assertions) {
                        re_log::error!("Failed to load SpaceView blueprint: {err}.");
                    } else {
                        re_log::debug!("Failed to load SpaceView blueprint: {err}.");
                    }
                }
            })
            .ok()?;

        let space_origin = space_origin.map_or_else(EntityPath::root, |origin| origin.0.into());

        let class_identifier: SpaceViewClassIdentifier = class_identifier.0.as_str().into();

        let display_name = display_name.map(|v| v.0.to_string());

        let queries = contents
            .unwrap_or_default()
            .0
            .into_iter()
            .map(DataQueryId::from)
            .filter_map(|id| DataQueryBlueprint::try_from_db(id, blueprint_db, class_identifier))
            .collect();

        let entities_determined_by_user = entities_determined_by_user.unwrap_or_default().0;

        let visible = visible.map_or(true, |v| v.0);

        Some(Self {
            id,
            display_name,
            class_identifier,
            space_origin,
            queries,
            entities_determined_by_user,
            visible,
        })
    }

    /// Persist the entire [`SpaceViewBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`SpaceViewBlueprint`] was created with [`Self::new`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let timepoint = TimePoint::timeless();

        let Self {
            id,
            display_name,
            class_identifier,
            space_origin,
            queries,
            entities_determined_by_user,
            visible,
        } = self;

        let mut arch =
            re_types::blueprint::archetypes::SpaceViewBlueprint::new(class_identifier.as_str())
                .with_space_origin(space_origin)
                .with_entities_determined_by_user(*entities_determined_by_user)
                .with_contents(queries.iter().map(|q| q.id))
                .with_visible(*visible);

        if let Some(display_name) = display_name {
            arch = arch.with_display_name(display_name.clone());
        }

        let mut deltas = vec![];

        if let Ok(row) =
            DataRow::from_archetype(RowId::new(), timepoint.clone(), id.as_entity_path(), &arch)
        {
            deltas.push(row);
        }

        for query in &self.queries {
            query.save_to_blueprint_store(ctx);
        }

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                deltas,
            ));
    }

    /// Creates a new [`SpaceViewBlueprint`] with a the same contents, but a different [`SpaceViewId`]
    ///
    /// Also duplicates all of the queries in the space view.
    pub fn duplicate(&self) -> Self {
        Self {
            id: SpaceViewId::random(),
            display_name: self.display_name.clone(),
            class_identifier: self.class_identifier,
            space_origin: self.space_origin.clone(),
            queries: self.queries.iter().map(|q| q.duplicate()).collect(),
            entities_determined_by_user: self.entities_determined_by_user,
            visible: self.visible,
        }
    }

    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        let clear = Clear::recursive();
        ctx.save_blueprint_component(&self.entity_path(), clear.is_recursive);

        for query in &self.queries {
            query.clear(ctx);
        }
    }

    #[inline]
    pub fn set_entity_determined_by_user(&self, ctx: &ViewerContext<'_>) {
        if !self.entities_determined_by_user {
            let component = EntitiesDeterminedByUser(true);
            ctx.save_blueprint_component(&self.entity_path(), component);
        }
    }

    #[inline]
    pub fn set_display_name(&self, name: Option<String>, ctx: &ViewerContext<'_>) {
        if name != self.display_name {
            match name {
                Some(name) => {
                    let component = Name(name.into());
                    ctx.save_blueprint_component(&self.entity_path(), component);
                }
                None => {
                    ctx.save_empty_blueprint_component::<Name>(&self.entity_path());
                }
            }
        }
    }

    #[inline]
    pub fn set_origin(&self, origin: &EntityPath, ctx: &ViewerContext<'_>) {
        if origin != &self.space_origin {
            let component = SpaceViewOrigin(origin.into());
            ctx.save_blueprint_component(&self.entity_path(), component);
        }
    }

    #[inline]
    pub fn set_visible(&self, visible: bool, ctx: &ViewerContext<'_>) {
        if visible != self.visible {
            let component = Visible(visible);
            ctx.save_blueprint_component(&self.entity_path(), component);
        }
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

    pub fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        view_state: &mut dyn SpaceViewState,
        view_props: &mut EntityPropertyMap,
    ) {
        while ScreenshotProcessor::next_readback_result(
            ctx.render_ctx,
            self.id.gpu_readback_id(),
            |data, extent, mode| self.handle_pending_screenshots(data, extent, mode),
        )
        .is_some()
        {}

        let query_result = ctx.lookup_query_result(self.query_id()).clone();

        let mut per_system_entities = PerSystemEntities::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
                    for system in &result.visualizers {
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
            view_props,
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
        fn is_safe_filename_char(c: char) -> bool {
            c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')
        }
        let safe_display_name = self
            .display_name_or_default()
            .0
            .replace(|c: char| !is_safe_filename_char(c), "");
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

    pub(crate) fn execute_systems<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        latest_at: TimeInt,
        highlights: SpaceViewHighlights,
    ) -> (ViewQuery<'a>, SystemExecutionOutput) {
        re_tracing::profile_function!(self.class_identifier.as_str());

        let class = self.class(ctx.space_view_class_registry);

        let query_result = ctx.lookup_query_result(self.query_id());

        let mut per_system_data_results = PerSystemDataResults::default();
        {
            re_tracing::profile_scope!("per_system_data_results");

            query_result.tree.visit(&mut |handle| {
                if let Some(result) = query_result.tree.lookup_result(handle) {
                    for system in &result.visualizers {
                        per_system_data_results
                            .entry(*system)
                            .or_default()
                            .push(result);
                    }
                }
            });
        }

        let query = re_viewer_context::ViewQuery {
            space_view_id: self.id,
            space_origin: &self.space_origin,
            per_system_data_results,
            timeline: *ctx.rec_cfg.time_ctrl.read().timeline(),
            latest_at,
            highlights,
        };

        let system_output = create_and_run_space_view_systems(ctx, class.identifier(), &query);

        (query, system_output)
    }

    pub(crate) fn scene_ui(
        &self,
        view_state: &mut dyn SpaceViewState,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) {
        re_tracing::profile_function!();

        let class = self.class(ctx.space_view_class_registry);

        let root_data_result = self.root_data_result(ctx.store_context);
        let props = root_data_result
            .individual_properties()
            .cloned()
            .unwrap_or_default();

        ui.scope(|ui| {
            class.ui(ctx, ui, view_state, &props, query, system_output);
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
            .map(|result| result.value.0);

        let accumulated_properties = individual_properties.clone().unwrap_or_else(|| {
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
            visualizers: Default::default(),
            is_group: true,
            direct_included: true,
            property_overrides: Some(PropertyOverrides {
                accumulated_properties,
                individual_properties,
                override_path: entity_path,
            }),
        }
    }

    // TODO(jleibs): Get rid of mut by sending blueprint update
    pub fn add_entity_exclusion(&self, ctx: &ViewerContext<'_>, rule: EntityPathRule) {
        if let Some(query) = self.queries.first() {
            query.add_entity_exclusion(ctx, rule);
        }
        self.set_entity_determined_by_user(ctx);
    }

    // TODO(jleibs): Get rid of mut by sending blueprint update
    pub fn add_entity_inclusion(&self, ctx: &ViewerContext<'_>, rule: EntityPathRule) {
        if let Some(query) = self.queries.first() {
            query.add_entity_inclusion(ctx, rule);
        }
        self.set_entity_determined_by_user(ctx);
    }

    pub fn remove_filter_rule_for(&self, ctx: &ViewerContext<'_>, ent_path: &EntityPath) {
        if let Some(query) = self.queries.first() {
            query.remove_filter_rule_for(ctx, ent_path);
        }
        self.set_entity_determined_by_user(ctx);
    }

    pub fn entity_path_filter(&self) -> EntityPathFilter {
        self.queries
            .iter()
            .map(|q| q.entity_path_filter.clone())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use re_entity_db::EntityDb;
    use re_log_types::{DataCell, DataRow, EntityPathFilter, RowId, StoreId, TimePoint};
    use re_space_view::{DataQuery as _, PropertyResolver as _};
    use re_types::archetypes::Points3D;
    use re_viewer_context::{
        IndicatorMatchingEntities, PerVisualizer, StoreContext, VisualizableEntities,
    };

    use super::*;

    fn save_override(props: EntityProperties, path: &EntityPath, store: &mut EntityDb) {
        let component = EntityPropertiesComponent(props);
        let row = DataRow::from_cells1_sized(
            RowId::new(),
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
        let mut recording = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        let mut blueprint = EntityDb::new(StoreId::random(re_log_types::StoreKind::Blueprint));

        let points = Points3D::new(vec![[1.0, 2.0, 3.0]]);

        for path in [
            "parent".into(),
            "parent/skip/child1".into(),
            "parent/skip/child2".into(),
        ] {
            let row = DataRow::from_archetype(RowId::new(), TimePoint::timeless(), path, &points)
                .unwrap();
            recording.add_data_row(row).ok();
        }

        let space_view = SpaceViewBlueprint::new(
            "3D".into(),
            &EntityPath::root(),
            DataQueryBlueprint::new(
                "3D".into(),
                EntityPathFilter::parse_forgiving(
                    r"
                    + parent
                    + parent/skip/child1
                    + parent/skip/child2
                ",
                ),
            ),
        );

        let auto_properties = Default::default();

        let mut visualizable_entities = PerVisualizer::<VisualizableEntities>::default();
        visualizable_entities
            .0
            .entry("Points3D".into())
            .or_insert_with(|| {
                VisualizableEntities(
                    [
                        EntityPath::from("parent"),
                        EntityPath::from("parent/skipped/child1"),
                    ]
                    .into_iter()
                    .collect(),
                )
            });
        let indicator_matching_entities_per_visualizer = PerVisualizer::<IndicatorMatchingEntities>(
            visualizable_entities
                .0
                .iter()
                .map(|(id, entities)| {
                    (
                        *id,
                        IndicatorMatchingEntities(entities.iter().map(|e| e.hash()).collect()),
                    )
                })
                .collect(),
        );

        let query = space_view.queries.first().unwrap();

        let resolver = query.build_resolver(space_view.id, &auto_properties);

        // No overrides set. Everybody has default values.
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let mut query_result = query.execute_query(
                &ctx,
                &visualizable_entities,
                &indicator_matching_entities_per_visualizer,
            );
            resolver.update_overrides(&ctx, &mut query_result);

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
                assert_eq!(
                    result.accumulated_properties(),
                    &EntityProperties::default(),
                );
            }

            // Now, override visibility on parent but not group
            let mut overrides = parent.individual_properties().cloned().unwrap_or_default();
            overrides.visible = false;

            save_override(overrides, parent.override_path().unwrap(), &mut blueprint);
        }

        // Parent is not visible, but children are
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let mut query_result = query.execute_query(
                &ctx,
                &visualizable_entities,
                &indicator_matching_entities_per_visualizer,
            );
            resolver.update_overrides(&ctx, &mut query_result);

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

            assert!(!parent.accumulated_properties().visible);

            for result in [child1, child2] {
                assert!(result.accumulated_properties().visible);
            }

            // Override visibility on parent group
            let mut overrides = parent_group
                .individual_properties()
                .cloned()
                .unwrap_or_default();
            overrides.visible = false;

            save_override(
                overrides,
                parent_group.override_path().unwrap(),
                &mut blueprint,
            );
        }

        // Nobody is visible
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let mut query_result = query.execute_query(
                &ctx,
                &visualizable_entities,
                &indicator_matching_entities_per_visualizer,
            );
            resolver.update_overrides(&ctx, &mut query_result);

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
                assert!(!result.accumulated_properties().visible);
            }
        }

        // Override visible range on root
        {
            let root = space_view.root_data_result(&StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            });
            let mut overrides = root.individual_properties().cloned().unwrap_or_default();
            overrides.visible_history.enabled = true;
            overrides.visible_history.nanos = VisibleHistory::ALL;

            save_override(overrides, root.override_path().unwrap(), &mut blueprint);
        }

        // Everyone has visible history
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let mut query_result = query.execute_query(
                &ctx,
                &visualizable_entities,
                &indicator_matching_entities_per_visualizer,
            );
            resolver.update_overrides(&ctx, &mut query_result);

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
                assert!(result.accumulated_properties().visible_history.enabled);
                assert_eq!(
                    result.accumulated_properties().visible_history.nanos,
                    VisibleHistory::ALL
                );
            }

            let mut overrides = child2.individual_properties().cloned().unwrap_or_default();
            overrides.visible_history.enabled = true;

            save_override(overrides, child2.override_path().unwrap(), &mut blueprint);
        }

        // Child2 has its own visible history
        {
            let ctx = StoreContext {
                blueprint: &blueprint,
                recording: Some(&recording),
                all_recordings: vec![],
            };

            let mut query_result = query.execute_query(
                &ctx,
                &visualizable_entities,
                &indicator_matching_entities_per_visualizer,
            );
            resolver.update_overrides(&ctx, &mut query_result);

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
                assert!(result.accumulated_properties().visible_history.enabled);
                assert_eq!(
                    result.accumulated_properties().visible_history.nanos,
                    VisibleHistory::ALL
                );
            }

            assert!(child2.accumulated_properties().visible_history.enabled);
            assert_eq!(
                child2.accumulated_properties().visible_history.nanos,
                VisibleHistory::OFF
            );
        }
    }
}
