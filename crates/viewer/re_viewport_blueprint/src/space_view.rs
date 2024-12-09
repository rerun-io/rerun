use std::sync::Arc;

use ahash::HashMap;
use itertools::{FoldWhile, Itertools};
use parking_lot::Mutex;
use re_types::{ComponentDescriptor, SpaceViewClassIdentifier};

use re_chunk::{Chunk, RowId};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath};
use re_log_types::{EntityPathSubs, Timeline};
use re_types::{
    blueprint::{
        archetypes::{self as blueprint_archetypes},
        components::{self as blueprint_components, SpaceViewOrigin, Visible},
    },
    components::Name,
};
use re_types_core::Archetype as _;
use re_viewer_context::{
    ContentsName, QueryRange, RecommendedSpaceView, SpaceViewClass, SpaceViewClassRegistry,
    SpaceViewId, SpaceViewState, StoreContext, SystemCommand, SystemCommandSender as _,
    ViewContext, ViewStates, ViewerContext, VisualizerCollection,
};

use crate::{SpaceViewContents, ViewProperty};

/// A view of a space.
///
/// Note: [`SpaceViewBlueprint`] doesn't implement Clone because it stores an internal
/// uuid used for identifying the path of its data in the blueprint store. It's ambiguous
/// whether the intent is for a clone to write to the same place.
///
/// If you want a new space view otherwise identical to an existing one, use
/// `re_viewport::ViewportBlueprint::duplicate_space_view`.
#[derive(Clone, Debug)]
pub struct SpaceViewBlueprint {
    pub id: SpaceViewId,
    pub display_name: Option<String>,
    class_identifier: SpaceViewClassIdentifier,

    /// The "anchor point" of this space view.
    /// The transform at this path forms the reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_origin: EntityPath,

    /// The content of this space view as defined by its queries.
    pub contents: SpaceViewContents,

    /// True if this space view is visible in the UI.
    pub visible: bool,

    /// Path where these space views defaults can be found.
    pub defaults_path: EntityPath,

    /// Pending blueprint writes for nested components from duplicate.
    pending_writes: Vec<Chunk>,
}

impl SpaceViewBlueprint {
    /// Path at which a view writes defaults for components.
    pub fn defaults_path(view_id: SpaceViewId) -> EntityPath {
        view_id.as_entity_path().join(&"defaults".into())
    }

    /// Creates a new [`SpaceViewBlueprint`] with a single [`SpaceViewContents`].
    ///
    /// This [`SpaceViewBlueprint`] is ephemeral. If you want to make it permanent you
    /// must call [`Self::save_to_blueprint_store`].
    pub fn new(
        space_view_class: SpaceViewClassIdentifier,
        recommended: RecommendedSpaceView,
    ) -> Self {
        let id = SpaceViewId::random();

        Self {
            display_name: None,
            class_identifier: space_view_class,
            id,
            space_origin: recommended.origin,
            contents: SpaceViewContents::new(id, space_view_class, recommended.query_filter),
            visible: true,
            defaults_path: Self::defaults_path(id),
            pending_writes: Default::default(),
        }
    }

    /// Placeholder name displayed in the UI if the user hasn't explicitly named the space view.
    pub fn missing_name_placeholder(&self) -> String {
        let entity_path = self
            .space_origin
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

    /// Returns this space view's display name
    ///
    /// When returning [`ContentsName::Placeholder`], the UI should display the resulting name using
    /// `re_ui::LabelStyle::Unnamed`.
    pub fn display_name_or_default(&self) -> ContentsName {
        self.display_name.clone().map_or_else(
            || ContentsName::Placeholder(self.missing_name_placeholder()),
            ContentsName::Named,
        )
    }

    /// Attempt to load a [`SpaceViewBlueprint`] from the blueprint store.
    pub fn try_from_db(
        id: SpaceViewId,
        blueprint_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<Self> {
        re_tracing::profile_function!();

        let results = blueprint_db.storage_engine().cache().latest_at(
            query,
            &id.as_entity_path(),
            blueprint_archetypes::SpaceViewBlueprint::all_components().iter(),
        );

        // This is a required component. Note that when loading space-views we crawl the subtree and so
        // cleared empty space-views paths may exist transiently. The fact that they have an empty class_identifier
        // is the marker that the have been cleared and not an error.
        let class_identifier =
            results.component_instance::<blueprint_components::SpaceViewClass>(0)?;

        let blueprint_archetypes::SpaceViewBlueprint {
            class_identifier,
            display_name,
            space_origin,
            visible,
        } = blueprint_archetypes::SpaceViewBlueprint {
            class_identifier,
            display_name: results.component_instance::<Name>(0),
            space_origin: results.component_instance::<SpaceViewOrigin>(0),
            visible: results.component_instance::<Visible>(0),
        };

        let space_origin = space_origin.map_or_else(EntityPath::root, |origin| origin.0.into());
        let class_identifier: SpaceViewClassIdentifier = class_identifier.0.as_str().into();
        let display_name = display_name.map(|v| v.0.to_string());

        let space_env = EntityPathSubs::new_with_origin(&space_origin);

        let content = SpaceViewContents::from_db_or_default(
            id,
            blueprint_db,
            query,
            class_identifier,
            &space_env,
        );
        let visible = visible.map_or(true, |v| *v.0);
        let defaults_path = id.as_entity_path().join(&"defaults".into());

        Some(Self {
            id,
            display_name,
            class_identifier,
            space_origin,
            contents: content,
            visible,
            defaults_path,
            pending_writes: Default::default(),
        })
    }

    /// Persist the entire [`SpaceViewBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`SpaceViewBlueprint`] was created with [`Self::new`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let timepoint = ctx.store_context.blueprint_timepoint_for_writes();

        let Self {
            id,
            display_name,
            class_identifier,
            space_origin,
            contents,
            visible,
            defaults_path: _,
            pending_writes,
        } = self;

        let mut arch = blueprint_archetypes::SpaceViewBlueprint::new(class_identifier.as_str())
            .with_space_origin(space_origin)
            .with_visible(*visible);

        if let Some(display_name) = display_name {
            arch = arch.with_display_name(display_name.clone());
        }

        // Start with the pending writes, which explicitly filtered out the `SpaceViewBlueprint`
        // components from the top level.
        let mut deltas = pending_writes.clone();

        // Add all the additional components from the archetype
        if let Ok(chunk) = Chunk::builder(id.as_entity_path())
            .with_archetype(RowId::new(), timepoint.clone(), &arch)
            .build()
        {
            deltas.push(chunk);
        }

        contents.save_to_blueprint_store(ctx);

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                deltas,
            ));
    }

    /// Creates a new [`SpaceViewBlueprint`] with the same contents, but a different [`SpaceViewId`]
    ///
    /// Also duplicates all the queries in the space view.
    pub fn duplicate(&self, store_context: &StoreContext<'_>, query: &LatestAtQuery) -> Self {
        let mut pending_writes = Vec::new();
        let blueprint = store_context.blueprint;
        let blueprint_engine = blueprint.storage_engine();

        let current_path = self.entity_path();
        let new_id = SpaceViewId::random();
        let new_path = new_id.as_entity_path();

        // Create pending write operations to duplicate the entire subtree
        // TODO(jleibs): This should be a helper somewhere.
        if let Some(tree) = blueprint.tree().subtree(&current_path) {
            tree.visit_children_recursively(|path| {
                let sub_path: EntityPath = new_path
                    .iter()
                    .chain(&path[current_path.len()..])
                    .cloned()
                    .collect();

                let chunk = Chunk::builder(sub_path)
                    .with_row(
                        RowId::new(),
                        store_context.blueprint_timepoint_for_writes(),
                        blueprint_engine
                            .store()
                            .all_components_on_timeline(&query.timeline(), path)
                            .into_iter()
                            .flat_map(|v| v.into_iter())
                            // It's important that we don't include the SpaceViewBlueprint's components
                            // since those will be updated separately and may contain different data.
                            .filter(|component_name| {
                                *path != current_path
                                    || !blueprint_archetypes::SpaceViewBlueprint::all_components()
                                        .iter()
                                        .any(|descr| descr.component_name == *component_name)
                            })
                            .filter_map(|component_name| {
                                let array = blueprint_engine
                                    .cache()
                                    .latest_at(query, path, [component_name])
                                    .component_batch_raw(&component_name);
                                array.map(|array| (ComponentDescriptor::new(component_name), array))
                            }),
                    )
                    .build();

                if let Ok(chunk) = chunk {
                    pending_writes.push(chunk);
                }
            });
        }

        // SpaceViewContents is saved as an archetype in the space view's entity hierarchy.
        // This means, that the above already copied the space view contents!
        let contents = SpaceViewContents::new(
            new_id,
            self.class_identifier,
            self.contents.entity_path_filter.clone(),
        );

        Self {
            id: new_id,
            display_name: self.display_name.clone(),
            class_identifier: self.class_identifier,
            space_origin: self.space_origin.clone(),
            contents,
            visible: self.visible,
            defaults_path: self.defaults_path.clone(),
            pending_writes,
        }
    }

    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        // We can't delete the entity, because we need to support undo.
        // TODO(#8249): configure blueprint GC to remove this entity if all that remains is the recursive clear.
        ctx.save_blueprint_archetype(
            &self.entity_path(),
            &re_types::archetypes::Clear::recursive(),
        );
    }

    #[inline]
    pub fn set_display_name(&self, ctx: &ViewerContext<'_>, name: Option<String>) {
        if name != self.display_name {
            match name {
                Some(name) => {
                    let component = Name(name.into());
                    ctx.save_blueprint_component(&self.entity_path(), &component);
                }
                None => {
                    ctx.save_empty_blueprint_component::<Name>(&self.entity_path());
                }
            }
        }
    }

    #[inline]
    pub fn set_origin(&self, ctx: &ViewerContext<'_>, origin: &EntityPath) {
        if origin != &self.space_origin {
            let component = SpaceViewOrigin(origin.into());
            ctx.save_blueprint_component(&self.entity_path(), &component);
        }
    }

    #[inline]
    pub fn set_visible(&self, ctx: &ViewerContext<'_>, visible: bool) {
        if visible != self.visible {
            let component = Visible::from(visible);
            ctx.save_blueprint_component(&self.entity_path(), &component);
        }
    }

    pub fn class_identifier(&self) -> SpaceViewClassIdentifier {
        self.class_identifier
    }

    pub fn class<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    ) -> &'a dyn SpaceViewClass {
        space_view_class_registry.get_class_or_log_error(self.class_identifier)
    }

    #[inline]
    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    pub fn query_range(
        &self,
        blueprint: &EntityDb,
        blueprint_query: &LatestAtQuery,
        active_timeline: &Timeline,
        space_view_class_registry: &SpaceViewClassRegistry,
        view_state: &dyn SpaceViewState,
    ) -> QueryRange {
        // Visual time range works with regular overrides for the most part but it's a bit special:
        // * we need it for all entities unconditionally
        // * default does not vary per visualizer
        // * can't be specified in the chunk store
        // Here, we query the visual time range that serves as the default for all entities in this space.

        let property = ViewProperty::from_archetype::<blueprint_archetypes::VisibleTimeRanges>(
            blueprint,
            blueprint_query,
            self.id,
        );
        let ranges = property.component_array();

        let time_range = ranges.ok().flatten().and_then(|ranges| {
            blueprint_archetypes::VisibleTimeRanges { ranges }
                .range_for_timeline(active_timeline.name().as_str())
                .cloned()
        });
        time_range.map_or_else(
            || {
                let space_view_class =
                    space_view_class_registry.get_class_or_log_error(self.class_identifier);
                space_view_class.default_query_range(view_state)
            },
            |time_range| QueryRange::TimeRange(time_range.clone()),
        )
    }

    pub fn bundle_context_with_states<'a>(
        &'a self,
        ctx: &'a ViewerContext<'a>,
        view_states: &'a mut ViewStates,
    ) -> ViewContext<'a> {
        let class = ctx
            .space_view_class_registry
            .get_class_or_log_error(self.class_identifier());
        let view_state = view_states.get_mut_or_create(self.id, class);

        ViewContext {
            viewer_ctx: ctx,
            view_id: self.id,
            view_state,
            defaults_path: &self.defaults_path,
            visualizer_collection: self.visualizer_collection(ctx),
        }
    }

    pub fn bundle_context_with_state<'a>(
        &'a self,
        ctx: &'a ViewerContext<'a>,
        view_state: &'a dyn SpaceViewState,
    ) -> ViewContext<'a> {
        ViewContext {
            viewer_ctx: ctx,
            view_id: self.id,
            view_state,
            defaults_path: &self.defaults_path,
            visualizer_collection: self.visualizer_collection(ctx),
        }
    }

    fn visualizer_collection(&self, ctx: &ViewerContext<'_>) -> Arc<VisualizerCollection> {
        static VISUALIZER_FOR_CONTEXT: once_cell::sync::Lazy<
            Mutex<HashMap<SpaceViewClassIdentifier, Arc<VisualizerCollection>>>,
        > = once_cell::sync::Lazy::new(Default::default);

        VISUALIZER_FOR_CONTEXT
            .lock()
            .entry(self.class_identifier())
            .or_insert_with(|| {
                Arc::new(
                    ctx.space_view_class_registry
                        .new_visualizer_collection(self.class_identifier()),
                )
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use re_chunk::RowId;
    use re_entity_db::EntityDb;
    use re_log_types::{
        example_components::{MyColor, MyLabel, MyPoint},
        StoreId, StoreKind, TimePoint,
    };
    use re_types::{Component as _, ComponentName};
    use re_viewer_context::{
        test_context::TestContext, ApplicableEntities, DataResult, IndicatedEntities, OverridePath,
        PerVisualizer, StoreContext, VisualizableEntities,
    };

    use crate::space_view_contents::DataQueryPropertyResolver;

    use super::*;

    #[test]
    fn test_component_overrides() {
        let mut test_ctx = TestContext::default();
        let mut visualizable_entities = PerVisualizer::<VisualizableEntities>::default();

        // Set up a store DB with some entities.
        {
            let entity_paths: Vec<EntityPath> =
                ["parent", "parent/skipped/grandchild", "parent/child"]
                    .into_iter()
                    .map(Into::into)
                    .collect();
            for entity_path in &entity_paths {
                let chunk = Chunk::builder(entity_path.clone())
                    .with_component_batches(
                        RowId::new(),
                        TimePoint::default(),
                        [&[MyPoint::new(1.0, 2.0)] as _],
                    )
                    .build()
                    .unwrap();

                test_ctx
                    .recording_store
                    .add_chunk(&Arc::new(chunk))
                    .unwrap();
            }

            // All of them are visualizable with some arbitrary visualizer.
            visualizable_entities
                .0
                .entry("Points3D".into())
                .or_insert_with(|| VisualizableEntities(entity_paths.into_iter().collect()));
        }

        let applicable_entities = PerVisualizer::<ApplicableEntities>(
            visualizable_entities
                .0
                .iter()
                .map(|(id, entities)| (*id, ApplicableEntities(entities.iter().cloned().collect())))
                .collect(),
        );

        // Basic blueprint - a single space view that queries everything.
        let space_view = SpaceViewBlueprint::new("3D".into(), RecommendedSpaceView::root());
        let individual_override_root = space_view
            .contents
            .blueprint_entity_path
            .join(&DataResult::INDIVIDUAL_OVERRIDES_PREFIX.into());
        let recursive_override_root = space_view
            .contents
            .blueprint_entity_path
            .join(&DataResult::RECURSIVE_OVERRIDES_PREFIX.into());

        // Things needed to resolve properties:
        let indicated_entities_per_visualizer = PerVisualizer::<IndicatedEntities>::default(); // Don't care about indicated entities.
        let resolver = space_view.contents.build_resolver(
            &test_ctx.space_view_class_registry,
            &space_view,
            &applicable_entities,
            &visualizable_entities,
            &indicated_entities_per_visualizer,
        );

        struct Scenario {
            recursive_overrides: Vec<(EntityPath, Box<dyn re_types_core::ComponentBatch>)>,
            individual_overrides: Vec<(EntityPath, Box<dyn re_types_core::ComponentBatch>)>,
            expected_overrides: HashMap<EntityPath, HashMap<ComponentName, EntityPath>>,
        }

        let scenarios: Vec<Scenario> = vec![
            // No overrides.
            Scenario {
                recursive_overrides: Vec::new(),
                individual_overrides: Vec::new(),
                expected_overrides: HashMap::default(),
            },
            // Recursive override at parent entity.
            Scenario {
                recursive_overrides: vec![(
                    "parent".into(),
                    Box::new(MyLabel("parent_override".to_owned())),
                )],
                individual_overrides: Vec::new(),
                expected_overrides: HashMap::from([
                    (
                        "parent".into(),
                        HashMap::from([(
                            MyLabel::name(),
                            recursive_override_root.join(&"parent".into()),
                        )]),
                    ),
                    (
                        "parent/skipped".into(),
                        HashMap::from([(
                            MyLabel::name(),
                            recursive_override_root.join(&"parent".into()),
                        )]),
                    ),
                    (
                        "parent/skipped/grandchild".into(),
                        HashMap::from([(
                            MyLabel::name(),
                            recursive_override_root.join(&"parent".into()),
                        )]),
                    ),
                    (
                        "parent/child".into(),
                        HashMap::from([(
                            MyLabel::name(),
                            recursive_override_root.join(&"parent".into()),
                        )]),
                    ),
                ]),
            },
            // Set a single individual.
            Scenario {
                recursive_overrides: Vec::new(),
                individual_overrides: vec![(
                    "parent".into(),
                    Box::new(MyLabel("parent_individual".to_owned())),
                )],
                expected_overrides: HashMap::from([(
                    "parent".into(),
                    HashMap::from([(
                        MyLabel::name(),
                        individual_override_root.join(&"parent".into()),
                    )]),
                )]),
            },
            // Recursive override, partially shadowed by individual.
            Scenario {
                recursive_overrides: vec![
                    (
                        "parent/skipped".into(),
                        Box::new(MyLabel("parent_individual".to_owned())),
                    ),
                    (
                        "parent/skipped".into(),
                        Box::new(MyColor::from_rgb(0, 1, 2)),
                    ),
                ],
                individual_overrides: vec![(
                    "parent/skipped/grandchild".into(),
                    Box::new(MyColor::from_rgb(1, 2, 3)),
                )],
                expected_overrides: HashMap::from([
                    (
                        "parent/skipped".into(),
                        HashMap::from([
                            (
                                MyLabel::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                            (
                                MyColor::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                        ]),
                    ),
                    (
                        "parent/skipped/grandchild".into(),
                        HashMap::from([
                            (
                                MyLabel::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                            (
                                MyColor::name(),
                                individual_override_root.join(&"parent/skipped/grandchild".into()),
                            ),
                        ]),
                    ),
                ]),
            },
            // Recursive override, partially shadowed by another recursive override.
            Scenario {
                recursive_overrides: vec![
                    (
                        "parent/skipped".into(),
                        Box::new(MyLabel("parent_individual".to_owned())),
                    ),
                    (
                        "parent/skipped".into(),
                        Box::new(MyColor::from_rgb(0, 1, 2)),
                    ),
                    (
                        "parent/skipped/grandchild".into(),
                        Box::new(MyColor::from_rgb(3, 2, 1)),
                    ),
                ],
                individual_overrides: Vec::new(),
                expected_overrides: HashMap::from([
                    (
                        "parent/skipped".into(),
                        HashMap::from([
                            (
                                MyLabel::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                            (
                                MyColor::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                        ]),
                    ),
                    (
                        "parent/skipped/grandchild".into(),
                        HashMap::from([
                            (
                                MyLabel::name(),
                                recursive_override_root.join(&"parent/skipped".into()),
                            ),
                            (
                                MyColor::name(),
                                recursive_override_root.join(&"parent/skipped/grandchild".into()),
                            ),
                        ]),
                    ),
                ]),
            },
        ];

        for (
            i,
            Scenario {
                recursive_overrides,
                individual_overrides,
                expected_overrides,
            },
        ) in scenarios.into_iter().enumerate()
        {
            // Reset blueprint store for each scenario.
            test_ctx.blueprint_store = EntityDb::new(StoreId::random(StoreKind::Blueprint));

            let mut add_to_blueprint =
                |path: &EntityPath, batch: &dyn re_types_core::ComponentBatch| {
                    let chunk = Chunk::builder(path.clone())
                        .with_component_batch(RowId::new(), TimePoint::default(), batch as _)
                        .build()
                        .unwrap();

                    test_ctx
                        .blueprint_store
                        .add_chunk(&Arc::new(chunk))
                        .unwrap();
                };

            // log individual and override components as instructed.
            for (entity_path, batch) in recursive_overrides {
                add_to_blueprint(&recursive_override_root.join(&entity_path), batch.as_ref());
            }
            for (entity_path, batch) in individual_overrides {
                add_to_blueprint(&individual_override_root.join(&entity_path), batch.as_ref());
            }

            // Set up a store query and update the overrides.
            let query_result = update_overrides(
                &test_ctx,
                &space_view.contents,
                &visualizable_entities,
                &resolver,
            );

            // Extract component overrides for testing.
            let mut visited: HashMap<EntityPath, HashMap<ComponentName, EntityPath>> =
                HashMap::default();
            query_result.tree.visit(&mut |node| {
                let result = &node.data_result;
                let resolved_component_overrides =
                    &result.property_overrides.resolved_component_overrides;
                if !resolved_component_overrides.is_empty() {
                    visited.insert(
                        result.entity_path.clone(),
                        resolved_component_overrides
                            .iter()
                            .map(|(component_name, OverridePath { store_kind, path })| {
                                assert_eq!(*store_kind, StoreKind::Blueprint);
                                (*component_name, path.clone())
                            })
                            .collect(),
                    );
                }
                true
            });

            assert_eq!(visited, expected_overrides, "Scenario {i}");
        }
    }

    fn update_overrides(
        test_ctx: &TestContext,
        contents: &SpaceViewContents,
        visualizable_entities: &PerVisualizer<VisualizableEntities>,
        resolver: &DataQueryPropertyResolver<'_>,
    ) -> re_viewer_context::DataQueryResult {
        let store_ctx = StoreContext {
            app_id: re_log_types::ApplicationId::unknown(),
            blueprint: &test_ctx.blueprint_store,
            default_blueprint: None,
            recording: &test_ctx.recording_store,
            bundle: &Default::default(),
            caches: &Default::default(),
            hub: &re_viewer_context::StoreHub::test_hub(),
        };

        let mut query_result = contents.execute_query(&store_ctx, visualizable_entities);
        let mut view_states = ViewStates::default();

        test_ctx.run_in_egui_central_panel(|ctx, _ui| {
            resolver.update_overrides(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                ctx.rec_cfg.time_ctrl.read().timeline(),
                ctx.space_view_class_registry,
                &mut query_result,
                &mut view_states,
            );
        });

        query_result
    }
}
