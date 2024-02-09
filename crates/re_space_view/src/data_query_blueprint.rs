use ahash::HashMap;
use slotmap::SlotMap;
use smallvec::SmallVec;

use re_entity_db::{
    external::re_data_store::LatestAtQuery, EntityDb, EntityProperties, EntityPropertiesComponent,
    EntityPropertyMap, EntityTree,
};
use re_log_types::{
    path::RuleEffect, DataRow, EntityPath, EntityPathFilter, EntityPathRule, RowId, StoreKind,
};
use re_types_core::{archetypes::Clear, components::VisualizerOverrides, ComponentName};
use re_viewer_context::{
    blueprint_timepoint_for_writes, DataQueryId, DataQueryResult, DataResult, DataResultHandle,
    DataResultNode, DataResultTree, IndicatedEntities, PerVisualizer, PropertyOverrides,
    SpaceViewClassIdentifier, StoreContext, SystemCommand, SystemCommandSender as _, ViewerContext,
    VisualizableEntities,
};

use crate::{
    blueprint::components::QueryExpressions, DataQuery, EntityOverrideContext, PropertyResolver,
    SpaceViewBlueprint,
};

/// An implementation of [`DataQuery`] that is built from a collection of [`QueryExpressions`]
///
/// During execution it will walk an [`EntityTree`] and return a [`DataResultTree`]
/// containing any entities that match a [`EntityPathFilter`]s.
///
/// Note: [`DataQueryBlueprint`] doesn't implement Clone because it stores an internal
/// uuid used for identifying the path of its data in the blueprint store. It's ambiguous
/// whether the intent is for a clone to write to the same place.
///
/// If you want a new space view otherwise identical to an existing one, use
/// [`DataQueryBlueprint::duplicate`].
pub struct DataQueryBlueprint {
    pub id: DataQueryId,
    pub space_view_class_identifier: SpaceViewClassIdentifier,
    pub entity_path_filter: EntityPathFilter,

    /// Pending blueprint writes for nested components from duplicate.
    pending_writes: Vec<DataRow>,
}

impl DataQueryBlueprint {
    pub fn is_equivalent(&self, other: &DataQueryBlueprint) -> bool {
        self.space_view_class_identifier
            .eq(&other.space_view_class_identifier)
            && self.entity_path_filter.eq(&other.entity_path_filter)
    }

    /// Returns true if this query will return a `DataQueryResult` that is guaranteed
    /// to contain any `EntityPath` that would be returned by the other query.
    ///
    /// This is a conservative estimate, and may return false negatives in some cases
    /// related to mixed inclusion/exclusion rules.
    ///
    /// It should never return false positives.
    pub fn fully_contains(&self, other: &DataQueryBlueprint) -> bool {
        // A query can't fully contain another if their space-view classes don't match
        if self.space_view_class_identifier != other.space_view_class_identifier {
            return false;
        }

        // Anything included by the other query is also included by this query
        self.entity_path_filter
            .fully_contains(&other.entity_path_filter)
    }
}

impl DataQueryBlueprint {
    /// Creates a new [`DataQueryBlueprint`].
    ///
    /// This [`DataQueryBlueprint`] is ephemeral. It must be saved by calling
    /// `save_to_blueprint_store` on the enclosing `SpaceViewBlueprint`.
    pub fn new(
        space_view_class_identifier: SpaceViewClassIdentifier,
        entity_path_filter: EntityPathFilter,
    ) -> Self {
        Self {
            id: DataQueryId::random(),
            space_view_class_identifier,
            entity_path_filter,
            pending_writes: Default::default(),
        }
    }

    /// Attempt to load a [`DataQueryBlueprint`] from the blueprint store.
    pub fn try_from_db(
        id: DataQueryId,
        blueprint_db: &EntityDb,
        query: &LatestAtQuery,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> Option<Self> {
        let expressions = blueprint_db
            .store()
            .query_latest_component::<QueryExpressions>(&id.as_entity_path(), query)
            .map(|c| c.value)?;

        let entity_path_filter = EntityPathFilter::from(&expressions);

        Some(Self {
            id,
            space_view_class_identifier,
            entity_path_filter,
            pending_writes: Default::default(),
        })
    }

    /// Persist the entire [`DataQueryBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`DataQueryBlueprint`] was created with [`Self::new`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        // Save any pending writes from a duplication.
        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                self.pending_writes.clone(),
            ));

        ctx.save_blueprint_component(
            &self.id.as_entity_path(),
            QueryExpressions::from(&self.entity_path_filter),
        );
    }

    /// Creates a new [`DataQueryBlueprint`] with a the same contents, but a different [`DataQueryId`]
    pub fn duplicate(&self, blueprint: &EntityDb, query: &LatestAtQuery) -> Self {
        let mut pending_writes = Vec::new();

        let current_path = self.id.as_entity_path();
        let new_id = DataQueryId::random();
        let new_path = new_id.as_entity_path();

        // Create pending write operations to duplicate the entire subtree
        // TODO(jleibs): This should be a helper somewhere.
        if let Some(tree) = blueprint.tree().subtree(&current_path) {
            tree.visit_children_recursively(&mut |path, info| {
                let sub_path: EntityPath = new_path
                    .iter()
                    .chain(&path[current_path.len()..])
                    .cloned()
                    .collect();

                if let Ok(row) = DataRow::from_cells(
                    RowId::new(),
                    blueprint_timepoint_for_writes(),
                    sub_path,
                    1,
                    info.components.keys().filter_map(|component| {
                        blueprint
                            .store()
                            .latest_at(query, path, *component, &[*component])
                            .and_then(|result| result.2[0].clone())
                    }),
                ) {
                    if row.num_cells() > 0 {
                        pending_writes.push(row);
                    }
                }
            });
        }

        Self {
            id: new_id,
            space_view_class_identifier: self.space_view_class_identifier,
            entity_path_filter: self.entity_path_filter.clone(),
            pending_writes,
        }
    }

    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        let clear = Clear::recursive();
        ctx.save_blueprint_component(&self.id.as_entity_path(), clear.is_recursive);
    }

    pub fn build_resolver<'a>(
        &self,
        space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
        space_view: &'a SpaceViewBlueprint,
        auto_properties: &'a EntityPropertyMap,
        visualizable_entities_per_visualizer: &'a PerVisualizer<VisualizableEntities>,
        indicated_entities_per_visualizer: &'a PerVisualizer<IndicatedEntities>,
    ) -> DataQueryPropertyResolver<'a> {
        let base_override_root = self.id.as_entity_path().clone();
        let individual_override_root =
            base_override_root.join(&DataResult::INDIVIDUAL_OVERRIDES_PREFIX.into());
        let recursive_override_root =
            base_override_root.join(&DataResult::RECURSIVE_OVERRIDES_PREFIX.into());
        DataQueryPropertyResolver {
            space_view_class_registry,
            space_view,
            auto_properties,
            default_stack: vec![space_view.entity_path(), self.id.as_entity_path()],
            individual_override_root,
            recursive_override_root,
            visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer,
        }
    }

    fn save_expressions(&self, ctx: &ViewerContext<'_>, entity_path_filter: &EntityPathFilter) {
        let timepoint = blueprint_timepoint_for_writes();

        let expressions_component = QueryExpressions::from(entity_path_filter);

        let row = DataRow::from_cells1_sized(
            RowId::new(),
            self.id.as_entity_path(),
            timepoint.clone(),
            1,
            [expressions_component],
        )
        .unwrap();

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                vec![row],
            ));
    }

    pub fn add_entity_exclusion(&self, ctx: &ViewerContext<'_>, rule: EntityPathRule) {
        // TODO(emilk): ignore new rule if it is already covered by existing rules (noop)
        let mut entity_path_filter = self.entity_path_filter.clone();
        entity_path_filter.add_rule(RuleEffect::Exclude, rule);
        self.save_expressions(ctx, &entity_path_filter);
    }

    pub fn add_entity_inclusion(&self, ctx: &ViewerContext<'_>, rule: EntityPathRule) {
        // TODO(emilk): ignore new rule if it is already covered by existing rules (noop)
        let mut entity_path_filter = self.entity_path_filter.clone();
        entity_path_filter.add_rule(RuleEffect::Include, rule);
        self.save_expressions(ctx, &entity_path_filter);
    }

    pub fn remove_filter_rule_for(&self, ctx: &ViewerContext<'_>, ent_path: &EntityPath) {
        let mut entity_path_filter = self.entity_path_filter.clone();
        entity_path_filter.remove_rule_for(ent_path);
        self.save_expressions(ctx, &entity_path_filter);
    }
}

impl DataQuery for DataQueryBlueprint {
    /// Build up the initial [`DataQueryResult`] for this [`DataQueryBlueprint`]
    ///
    /// Note that this result will not have any resolved [`PropertyOverrides`]. Those can
    /// be added by separately calling [`PropertyResolver::update_overrides`] on
    /// the result.
    fn execute_query(
        &self,
        ctx: &re_viewer_context::StoreContext<'_>,
        visualizable_entities_for_visualizer_systems: &PerVisualizer<VisualizableEntities>,
    ) -> DataQueryResult {
        re_tracing::profile_function!();

        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();

        let executor =
            QueryExpressionEvaluator::new(self, visualizable_entities_for_visualizer_systems);

        let root_handle = ctx.recording.and_then(|store| {
            re_tracing::profile_scope!("add_entity_tree_to_data_results_recursive");
            executor.add_entity_tree_to_data_results_recursive(store.tree(), &mut data_results)
        });

        DataQueryResult {
            id: self.id,
            tree: DataResultTree::new(data_results, root_handle),
        }
    }
}

/// Helper struct for executing the query from [`DataQueryBlueprint`]
///
/// This restructures the [`QueryExpressions`] into several sets that are
/// used to efficiently determine if we should continue the walk or switch
/// to a pure recursive evaluation.
struct QueryExpressionEvaluator<'a> {
    visualizable_entities_for_visualizer_systems: &'a PerVisualizer<VisualizableEntities>,
    entity_path_filter: EntityPathFilter,
}

impl<'a> QueryExpressionEvaluator<'a> {
    fn new(
        blueprint: &'a DataQueryBlueprint,
        visualizable_entities_for_visualizer_systems: &'a PerVisualizer<VisualizableEntities>,
    ) -> Self {
        re_tracing::profile_function!();

        Self {
            visualizable_entities_for_visualizer_systems,
            entity_path_filter: blueprint.entity_path_filter.clone(),
        }
    }

    fn add_entity_tree_to_data_results_recursive(
        &self,
        tree: &EntityTree,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
    ) -> Option<DataResultHandle> {
        // Early-out optimization
        if !self
            .entity_path_filter
            .is_anything_in_subtree_included(&tree.path)
        {
            return None;
        }

        // TODO(jleibs): If this space is disconnected, we should terminate here

        let entity_path = &tree.path;

        // Pre-compute our matches
        let any_match = self.entity_path_filter.is_included(entity_path);

        // Check for visualizers if this is a match.
        // Note that allowed prefixes that aren't matches can still create groups.
        let visualizable = any_match
            && self
                .visualizable_entities_for_visualizer_systems
                .values()
                .any(|ents| ents.contains(entity_path));

        let self_leaf = if visualizable || self.entity_path_filter.is_exact_included(entity_path) {
            // TODO(#5067): For now, we always start by setting visualizers to the full list of available visualizers.
            // This is currently important for evaluating auto-properties during the space-view `on_frame_start`, which
            // is called before the property-overrider has a chance to update this list.
            // This list will be updated below during `update_overrides_recursive` by calling `choose_default_visualizers`
            // on the space view.
            let available_visualizers = self
                .visualizable_entities_for_visualizer_systems
                .iter()
                .filter_map(|(visualizer, ents)| {
                    if ents.contains(entity_path) {
                        Some(*visualizer)
                    } else {
                        None
                    }
                })
                .collect();

            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    visualizers: available_visualizers,
                    is_group: false,
                    direct_included: any_match,
                    property_overrides: None,
                },
                children: Default::default(),
            }))
        } else {
            None
        };

        let maybe_self_iter = if let Some(self_leaf) = self_leaf {
            itertools::Either::Left(std::iter::once(self_leaf))
        } else {
            itertools::Either::Right(std::iter::empty())
        };

        let children: SmallVec<_> = maybe_self_iter
            .chain(tree.children.values().filter_map(|subtree| {
                self.add_entity_tree_to_data_results_recursive(subtree, data_results)
            }))
            .collect();

        // If the only child is the self-leaf, then we don't need to create a group
        if children.is_empty() || children.len() == 1 && self_leaf.is_some() {
            self_leaf
        } else {
            // The 'individual' properties of a group are the group overrides
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    visualizers: Default::default(),
                    is_group: true,
                    direct_included: any_match,
                    property_overrides: None,
                },
                children,
            }))
        }
    }
}

pub struct DataQueryPropertyResolver<'a> {
    space_view_class_registry: &'a re_viewer_context::SpaceViewClassRegistry,
    space_view: &'a SpaceViewBlueprint,
    auto_properties: &'a EntityPropertyMap,
    default_stack: Vec<EntityPath>,
    individual_override_root: EntityPath,
    recursive_override_root: EntityPath,
    visualizable_entities_per_visualizer: &'a PerVisualizer<VisualizableEntities>,
    indicated_entities_per_visualizer: &'a PerVisualizer<IndicatedEntities>,
}

impl DataQueryPropertyResolver<'_> {
    /// Helper function to build the [`EntityOverrideContext`] for this [`DataQuery`]
    ///
    /// The context is made up of 3 parts:
    ///  - The root properties are build by merging a stack of paths from the Blueprint Tree. This
    ///  may include properties from the `SpaceView` or `DataQuery`.
    ///  - The individual overrides are found by walking an override subtree under the `data_query/<id>/individual_overrides`
    ///  - The group overrides are found by walking an override subtree under the `data_query/<id>/group_overrides`
    fn build_override_context(
        &self,
        ctx: &StoreContext<'_>,
        query: &LatestAtQuery,
    ) -> EntityOverrideContext {
        re_tracing::profile_function!();

        // TODO(#4194): We always start the override context with the root_data_result from
        // the space-view. This isn't totally generic once we add container overrides, but it's a start.
        let mut root: EntityProperties = self
            .space_view
            .root_data_result(ctx, query)
            .property_overrides
            .map(|p| p.accumulated_properties.clone())
            .unwrap_or_default();

        for prefix in &self.default_stack {
            if let Some(overrides) = ctx
                .blueprint
                .store()
                .query_latest_component::<EntityPropertiesComponent>(prefix, query)
            {
                root = root.with_child(&overrides.value.0);
            }
        }

        // TODO(jleibs): Should pass through an initial `ComponentOverrides` here
        // if we were to support incrementally inheriting overrides from parent
        // contexts such as the `SpaceView` or `Container`.
        EntityOverrideContext {
            root,
            individual: self.resolve_entity_overrides_for_path(
                ctx,
                query,
                &self.individual_override_root,
            ),
            group: self.resolve_entity_overrides_for_path(
                ctx,
                query,
                &self.recursive_override_root,
            ),
        }
    }

    /// Find all of the overrides for a given path.
    ///
    /// These overrides are full entity-paths prefixed by the override root.
    ///
    /// For example the individual override for `world/points` in the context of the query-id `1234`
    /// would be found at: `data_query/1234/individual_overrides/world/points`.
    fn resolve_entity_overrides_for_path(
        &self,
        ctx: &StoreContext<'_>,
        query: &LatestAtQuery,
        override_root: &EntityPath,
    ) -> EntityPropertyMap {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut prop_map = self.auto_properties.clone();

        if let Some(tree) = blueprint.tree().subtree(override_root) {
            tree.visit_children_recursively(&mut |path: &EntityPath, _| {
                if let Some(props) = blueprint
                    .store()
                    .query_latest_component_quiet::<EntityPropertiesComponent>(path, query)
                {
                    let overridden_path =
                        EntityPath::from(&path.as_slice()[override_root.len()..path.len()]);
                    prop_map.update(overridden_path, props.value.0);
                }
            });
        }
        prop_map
    }

    /// Recursively walk the [`DataResultTree`] and update the [`PropertyOverrides`] for each node.
    ///
    /// This will accumulate the group properties at each step down the tree, and then finally merge
    /// with individual overrides at the leaves.
    fn update_overrides_recursive(
        &self,
        ctx: &StoreContext<'_>,
        query: &LatestAtQuery,
        query_result: &mut DataQueryResult,
        override_context: &EntityOverrideContext,
        accumulated: &EntityProperties,
        handle: DataResultHandle,
    ) {
        if let Some((child_handles, accumulated)) =
            query_result.tree.lookup_node_mut(handle).and_then(|node| {
                if node.data_result.is_group {
                    let overridden_properties = override_context
                        .group
                        .get_opt(&node.data_result.entity_path);

                    let accumulated_properties = if let Some(overridden) = overridden_properties {
                        accumulated.with_child(overridden)
                    } else {
                        accumulated.clone()
                    };

                    node.data_result.property_overrides = Some(PropertyOverrides {
                        individual_properties: overridden_properties.cloned(),
                        accumulated_properties: accumulated_properties.clone(),
                        component_overrides: Default::default(),
                        override_path: self
                            .recursive_override_root
                            .join(&node.data_result.entity_path),
                    });

                    Some((node.children.clone(), accumulated_properties))
                } else {
                    let overridden_properties = override_context
                        .individual
                        .get_opt(&node.data_result.entity_path);

                    let accumulated_properties = if let Some(overridden) = overridden_properties {
                        accumulated.with_child(overridden)
                    } else {
                        accumulated.clone()
                    };

                    let override_path = self
                        .individual_override_root
                        .join(&node.data_result.entity_path);

                    {
                        re_tracing::profile_scope!("Update visualizers from overrides");

                        // If the user has overridden the visualizers, update which visualizers are used.
                        if let Some(viz_override) = ctx
                            .blueprint
                            .store()
                            .query_latest_component::<VisualizerOverrides>(&override_path, query)
                            .map(|c| c.value)
                        {
                            node.data_result.visualizers =
                                viz_override.0.iter().map(|v| v.as_str().into()).collect();
                        } else {
                            // Otherwise ask the `SpaceViewClass` to choose.
                            node.data_result.visualizers = self
                                .space_view
                                .class(self.space_view_class_registry)
                                .choose_default_visualizers(
                                    &node.data_result.entity_path,
                                    self.visualizable_entities_per_visualizer,
                                    self.indicated_entities_per_visualizer,
                                );
                        }
                    }
                    let mut component_overrides: HashMap<ComponentName, (StoreKind, EntityPath)> =
                        Default::default();

                    if let Some(override_subtree) = ctx.blueprint.tree().subtree(&override_path) {
                        for component in override_subtree.entity.components.keys() {
                            if let Some(component_data) = ctx
                                .blueprint
                                .store()
                                .latest_at(query, &override_path, *component, &[*component])
                                .and_then(|(_, _, cells)| cells[0].clone())
                            {
                                if !component_data.is_empty() {
                                    component_overrides.insert(
                                        *component,
                                        (StoreKind::Blueprint, override_path.clone()),
                                    );
                                }
                            }
                        }
                    }

                    node.data_result.property_overrides = Some(PropertyOverrides {
                        individual_properties: overridden_properties.cloned(),
                        accumulated_properties: accumulated_properties.clone(),
                        component_overrides,
                        override_path,
                    });

                    None
                }
            })
        {
            for child in child_handles {
                self.update_overrides_recursive(
                    ctx,
                    query,
                    query_result,
                    override_context,
                    &accumulated,
                    child,
                );
            }
        }
    }
}

impl<'a> PropertyResolver for DataQueryPropertyResolver<'a> {
    /// Recursively walk the [`DataResultTree`] and update the [`PropertyOverrides`] for each node.
    fn update_overrides(
        &self,
        ctx: &StoreContext<'_>,
        query: &LatestAtQuery,
        query_result: &mut DataQueryResult,
    ) {
        re_tracing::profile_function!();
        let entity_overrides = self.build_override_context(ctx, query);

        if let Some(root) = query_result.tree.root_handle() {
            self.update_overrides_recursive(
                ctx,
                query,
                query_result,
                &entity_overrides,
                &entity_overrides.root,
                root,
            );
        }
    }
}

#[cfg(feature = "testing")]
#[cfg(test)]
mod tests {
    use re_entity_db::EntityDb;
    use re_log_types::{example_components::MyPoint, DataRow, RowId, StoreId, TimePoint, Timeline};
    use re_viewer_context::{StoreContext, VisualizableEntities};

    use super::*;

    #[test]
    fn test_query_results() {
        let mut recording = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        let blueprint = EntityDb::new(StoreId::random(re_log_types::StoreKind::Blueprint));

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);

        // Set up a store DB with some entities
        for entity_path in ["parent", "parent/skipped/child1", "parent/skipped/child2"] {
            let row_id = RowId::new();
            let point = MyPoint::new(1.0, 2.0);
            let row = DataRow::from_component_batches(
                row_id,
                timepoint.clone(),
                entity_path.into(),
                [&[point] as _],
            )
            .unwrap();

            recording.add_data_row(row).unwrap();
        }

        let mut visualizable_entities_for_visualizer_systems =
            PerVisualizer::<VisualizableEntities>::default();

        visualizable_entities_for_visualizer_systems
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

        let ctx = StoreContext {
            blueprint: &blueprint,
            recording: Some(&recording),
            all_recordings: vec![],
        };

        struct Scenario {
            filter: &'static str,
            outputs: Vec<&'static str>,
        }

        let scenarios: Vec<Scenario> = vec![
            Scenario {
                filter: "+ /**",
                outputs: vec![
                    "/**",
                    "/parent/**",
                    "/parent",
                    "/parent/skipped/**", // Not an exact match and not found in tree
                    "/parent/skipped/child1", // Only child 1 has visualizers
                ],
            },
            Scenario {
                filter: "+ parent/skipped/**",
                outputs: vec![
                    "/**",
                    "/parent/**",             // Only included because is a prefix
                    "/parent/skipped/**",     // Not an exact match and not found in tree
                    "/parent/skipped/child1", // Only child 1 has visualizers
                ],
            },
            Scenario {
                filter: r"+ parent
                          + parent/skipped/child2",
                outputs: vec![
                    "/**", // Trivial intermediate group -- could be collapsed
                    "/parent/**",
                    "/parent",
                    "/parent/skipped/**", // Trivial intermediate group -- could be collapsed
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                filter: r"+ parent/skipped
                          + parent/skipped/child2
                          + parent/**",
                outputs: vec![
                    "/**",
                    "/parent/**",
                    "/parent",
                    "/parent/skipped/**",
                    "/parent/skipped",        // Included because an exact match
                    "/parent/skipped/child1", // Included because an exact match
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                filter: r"+ parent/skipped
                          + parent/skipped/child2
                          + parent/**
                          - parent",
                outputs: vec![
                    "/**",
                    "/parent/**", // Parent leaf has been excluded
                    "/parent/skipped/**",
                    "/parent/skipped",        // Included because an exact match
                    "/parent/skipped/child1", // Included because an exact match
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                filter: r"+ parent/**
                          - parent/skipped/**",
                outputs: vec!["/**", "/parent"], // None of the children are hit since excluded
            },
            Scenario {
                filter: r"+ parent/**
                          + parent/skipped/child2
                          - parent/skipped/child1",
                outputs: vec![
                    "/**",
                    "/parent/**",
                    "/parent",
                    "/parent/skipped/**",
                    "/parent/skipped/child2", // No child1 since skipped.
                ],
            },
            Scenario {
                filter: r"+ not/found",
                // TODO(jleibs): Making this work requires merging the EntityTree walk with a minimal-coverage ExactMatchTree walk
                // not crucial for now until we expose a free-form UI for entering paths.
                // vec!["/**", "not/**", "not/found"]),
                outputs: vec![],
            },
        ];

        for (i, Scenario { filter, outputs }) in scenarios.into_iter().enumerate() {
            let query = DataQueryBlueprint {
                id: DataQueryId::random(),
                space_view_class_identifier: "3D".into(),
                entity_path_filter: EntityPathFilter::parse_forgiving(filter),
            };

            let indicated_entities_per_visualizer = PerVisualizer::<IndicatedEntities>(
                visualizable_entities_for_visualizer_systems
                    .iter()
                    .map(|(id, entities)| {
                        (*id, IndicatedEntities(entities.0.iter().cloned().collect()))
                    })
                    .collect(),
            );
            let query_result = query.execute_query(
                &ctx,
                &visualizable_entities_for_visualizer_systems,
                &indicated_entities_per_visualizer,
            );

            let mut visited = vec![];
            query_result.tree.visit(&mut |handle| {
                let result = query_result.tree.lookup_result(handle).unwrap();
                if result.is_group && result.entity_path == EntityPath::root() {
                    visited.push("/**".to_owned());
                } else if result.is_group {
                    visited.push(format!("{}/**", result.entity_path));
                } else {
                    visited.push(result.entity_path.to_string());
                }
            });

            assert_eq!(visited, outputs, "Scenario {i}, filter: {filter}");
        }
    }
}
