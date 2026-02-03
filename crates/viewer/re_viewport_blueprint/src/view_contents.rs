use std::sync::Arc;

use arrow::array::AsArray as _;
use nohash_hasher::{IntMap, IntSet};
use re_entity_db::external::re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityTree};
use re_log_types::path::RuleEffect;
use re_log_types::{
    EntityPath, EntityPathFilter, EntityPathHash, EntityPathSubs, ResolvedEntityPathFilter,
    ResolvedEntityPathRule, Timeline,
};
use re_mutex::Mutex;
use re_sdk_types::blueprint::components::{QueryExpression, VisualizerInstructionId};
use re_sdk_types::blueprint::{
    archetypes as blueprint_archetypes, components as blueprint_components,
};
use re_sdk_types::{Loggable as _, ViewClassIdentifier};
use re_viewer_context::{
    DataQueryResult, DataResult, DataResultHandle, DataResultNode, DataResultTree,
    IndicatedEntities, PerVisualizerType, PerVisualizerTypeInViewClass, QueryRange, ViewId,
    ViewSystemIdentifier, ViewerContext, VisualizableEntities, VisualizerComponentMappings,
    VisualizerInstruction,
};
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{ViewBlueprint, ViewProperty};

/// Data to be added to a view, built from a [`blueprint_archetypes::ViewContents`].
///
/// During execution, it will walk an [`EntityTree`] and return a [`DataResultTree`]
/// containing any entities that match a [`ResolvedEntityPathFilter`].
///
/// Note: [`ViewContents`] doesn't implement Clone because it depends on its parent's [`ViewId`]
/// used for identifying the path of its data in the blueprint store. It's ambiguous
/// whether the intent is for a clone to write to the same place.
///
/// If you want a new view otherwise identical to an existing one, use
/// [`ViewBlueprint::duplicate`].
#[derive(Clone, Debug)]
pub struct ViewContents {
    view_id: ViewId,

    view_class_identifier: ViewClassIdentifier,

    /// Deserialized entity path filter.
    ///
    /// Consider this read-only.
    entity_path_filter: ResolvedEntityPathFilter,

    /// Update entity path filter.
    ///
    /// Mutations go to this value and should be saved to the blueprint store when they occur.
    new_entity_path_filter: Arc<Mutex<ResolvedEntityPathFilter>>,
}

impl ViewContents {
    pub fn is_equivalent(&self, other: &Self) -> bool {
        self.view_class_identifier.eq(&other.view_class_identifier)
            && self.entity_path_filter.eq(&other.entity_path_filter)
    }

    /// Checks whether the results of this query "fully contains" the results of another query.
    ///
    /// If this returns `true` then the [`DataQueryResult`] returned by this query should always
    /// contain any [`EntityPath`] that would be included in the results of the other query.
    ///
    /// This is a conservative estimate, and may return `false` in situations where the
    /// query does in fact cover the other query. However, it should never return `true`
    /// in a case where the other query would not be fully covered.
    pub fn entity_path_filter_is_superset_of(&self, other: &Self) -> bool {
        // A query can't fully contain another if their view classes don't match
        if self.view_class_identifier != other.view_class_identifier {
            return false;
        }

        // Anything included by the other query is also included by this query
        self.entity_path_filter
            .is_superset_of(&other.entity_path_filter)
    }
}

impl ViewContents {
    /// Creates a new [`ViewContents`].
    ///
    /// This [`ViewContents`] is ephemeral. It must be saved by calling
    /// `save_to_blueprint_store` on the enclosing `ViewBlueprint`.
    pub fn new(
        view_id: ViewId,
        view_class_identifier: ViewClassIdentifier,
        entity_path_filter: ResolvedEntityPathFilter,
    ) -> Self {
        let new_entity_path_filter = Arc::new(Mutex::new(entity_path_filter.clone()));

        Self {
            view_id,
            view_class_identifier,
            entity_path_filter,
            new_entity_path_filter,
        }
    }

    /// Computes the override path for a given entity in a given view.
    pub fn base_override_path_for_entity(id: ViewId, entity_path: &EntityPath) -> EntityPath {
        blueprint_archetypes::ViewContents::blueprint_base_visualizer_path_for_entity(
            id.uuid(),
            entity_path,
        )
    }

    /// Attempt to load a [`ViewContents`] from the blueprint store.
    pub fn from_db_or_default(
        view_id: ViewId,
        blueprint_db: &EntityDb,
        query: &LatestAtQuery,
        view_class_identifier: ViewClassIdentifier,
        subst_env: &EntityPathSubs,
    ) -> Self {
        let property = ViewProperty::from_archetype::<blueprint_archetypes::ViewContents>(
            blueprint_db,
            query,
            view_id,
        );
        let expressions = match property.component_array_or_empty::<QueryExpression>(
            blueprint_archetypes::ViewContents::descriptor_query().component,
        ) {
            Ok(expressions) => expressions,

            Err(err) => {
                re_log::warn_once!(
                    "Failed to load ViewContents for {:?} from blueprint store at {:?}: {}",
                    view_id,
                    property.blueprint_store_path,
                    err
                );
                Default::default()
            }
        };
        let query = expressions.iter().map(|qe| qe.0.as_str());
        let entity_path_filter =
            EntityPathFilter::from_query_expressions(query).resolve_forgiving(subst_env);

        let new_entity_path_filter = Arc::new(Mutex::new(entity_path_filter.clone()));

        Self {
            view_id,
            view_class_identifier,
            entity_path_filter,
            new_entity_path_filter,
        }
    }

    /// Persist the entire [`ViewContents`] to the blueprint store.
    ///
    /// This only needs to be called if the [`ViewContents`] was created with [`Self::new`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        self.save_entity_path_filter_to_blueprint(ctx);
    }

    pub fn entity_path_filter(&self) -> &ResolvedEntityPathFilter {
        &self.entity_path_filter
    }

    /// Sets the entity path filter to the provided one.
    pub fn set_entity_path_filter(
        &self,
        ctx: &ViewerContext<'_>,
        entity_path_filter: ResolvedEntityPathFilter,
    ) {
        *self.new_entity_path_filter.lock() = entity_path_filter;
        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Perform arbitrary mutation on the entity path filter.
    ///
    /// Using this method avoids triggering multiple writes to the blueprint store.
    pub fn mutate_entity_path_filter(
        &self,
        ctx: &ViewerContext<'_>,
        f: impl FnOnce(&mut ResolvedEntityPathFilter),
    ) {
        f(&mut self.new_entity_path_filter.lock());
        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Remove a subtree and any existing rules that it would match.
    ///
    /// Because most-specific matches win, if we only add a subtree exclusion
    /// it can still be overridden by existing inclusions. This method ensures
    /// that not only do we add a subtree exclusion, but clear out any existing
    /// inclusions or (now redundant) exclusions that would match the subtree.
    pub fn remove_subtree_and_matching_rules(&self, ctx: &ViewerContext<'_>, path: EntityPath) {
        self.new_entity_path_filter
            .lock()
            .remove_subtree_and_matching_rules(path);

        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Directly add an exclusion rule to the [`EntityPathFilter`].
    ///
    /// This is a direct modification of the filter and will not do any simplification
    /// related to overlapping or conflicting rules.
    ///
    /// If you are trying to remove an entire subtree, prefer using [`Self::remove_subtree_and_matching_rules`].
    pub fn raw_add_entity_exclusion(&self, ctx: &ViewerContext<'_>, rule: ResolvedEntityPathRule) {
        self.new_entity_path_filter
            .lock()
            .add_rule(RuleEffect::Exclude, rule);

        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Directly add an inclusion rule to the [`EntityPathFilter`].
    ///
    /// This is a direct modification of the filter and will not do any simplification
    /// related to overlapping or conflicting rules.
    pub fn raw_add_entity_inclusion(&self, ctx: &ViewerContext<'_>, rule: ResolvedEntityPathRule) {
        self.new_entity_path_filter
            .lock()
            .add_rule(RuleEffect::Include, rule);

        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Remove rules for a given entity.
    pub fn remove_filter_rule_for(&self, ctx: &ViewerContext<'_>, ent_path: &EntityPath) {
        self.new_entity_path_filter.lock().remove_rule_for(ent_path);

        self.save_entity_path_filter_to_blueprint(ctx);
    }

    /// Save the entity path filter.
    fn save_entity_path_filter_to_blueprint(&self, ctx: &ViewerContext<'_>) {
        ViewProperty::from_archetype::<blueprint_archetypes::ViewContents>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            self.view_id,
        )
        .save_blueprint_component(
            ctx,
            &blueprint_archetypes::ViewContents::descriptor_query(),
            &self
                .new_entity_path_filter
                .lock()
                .iter_unresolved_expressions()
                .map(|s| blueprint_components::QueryExpression(s.into()))
                .collect::<Vec<_>>(),
        );
    }

    /// Build up the initial [`DataQueryResult`] for this [`ViewContents`]
    ///
    /// Note that this result will not have any resolved overrides. Those can
    /// be added by separately calling `DataQueryPropertyResolver::update_overrides` on
    /// the result.
    #[expect(clippy::too_many_arguments)]
    pub fn build_data_result_tree(
        &self,
        ctx: &re_viewer_context::StoreContext<'_>,
        active_timeline: Option<&Timeline>,
        view_class_registry: &re_viewer_context::ViewClassRegistry,
        blueprint_query: &LatestAtQuery,
        query_range: &QueryRange,
        visualizable_entities_per_visualizer: &PerVisualizerTypeInViewClass<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizerType<IndicatedEntities>,
        app_options: &re_viewer_context::AppOptions,
    ) -> DataQueryResult {
        re_tracing::profile_function!();

        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();

        let visualizers_per_entity =
            Self::visualizers_per_entity(visualizable_entities_per_visualizer);

        let executor = QueryExpressionEvaluator {
            visualizers_per_entity: &visualizers_per_entity,
            entity_path_filter: &self.entity_path_filter,
            view_id: self.view_id,
        };

        let mut num_matching_entities = 0;
        let mut num_visualized_entities = 0;
        let root_handle = {
            re_tracing::profile_scope!("add_entity_tree_to_data_results_recursive");
            executor.add_entity_tree_to_data_results_recursive(
                ctx.recording.tree(),
                &mut data_results,
                &mut num_matching_entities,
                &mut num_visualized_entities,
            )
        };

        // Query defaults for all the components that any visualizer in this view is interested in.
        let component_defaults = {
            re_tracing::profile_scope!("component_defaults");

            let visualizer_collection =
                view_class_registry.new_visualizer_collection(self.view_class_identifier);

            // Figure out which components are relevant.
            let mut components_for_defaults = IntSet::default();
            for (visualizer, entities) in visualizable_entities_per_visualizer.iter() {
                if entities.is_empty() {
                    continue;
                }
                let Ok(visualizer) = visualizer_collection.get_by_type_identifier(*visualizer)
                else {
                    continue;
                };
                components_for_defaults.extend(
                    visualizer
                        .visualizer_query_info(app_options)
                        .queried_components(),
                );
            }

            ctx.blueprint.latest_at(
                blueprint_query,
                &ViewBlueprint::defaults_path(self.view_id),
                components_for_defaults,
            )
        };

        let mut query_result = DataQueryResult {
            tree: DataResultTree::new(data_results, root_handle),
            num_matching_entities,
            num_visualized_entities,
            view_defaults: component_defaults,
        };

        // TODO(andreas): integrate with `add_entity_tree_to_data_results_recursive` above.
        let resolver = DataQueryPropertyResolver {
            view_query_range: query_range,
            view_class: view_class_registry.get_class_or_log_error(self.view_class_identifier),
            visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer,
        };

        resolver.update_overrides(
            ctx.blueprint,
            blueprint_query,
            active_timeline,
            &mut query_result,
        );

        query_result
    }

    fn visualizers_per_entity(
        visualizable_entities_for_visualizer_systems: &PerVisualizerTypeInViewClass<
            VisualizableEntities,
        >,
    ) -> IntMap<EntityPathHash, SmallVec<[ViewSystemIdentifier; 4]>> {
        re_tracing::profile_function!();

        let mut visualizers_per_entity = IntMap::default();
        for (visualizer, visualizable_entities) in
            visualizable_entities_for_visualizer_systems.iter()
        {
            for entity_path in visualizable_entities.keys() {
                visualizers_per_entity
                    .entry(entity_path.hash())
                    .or_insert_with(SmallVec::new)
                    .push(*visualizer);
            }
        }
        visualizers_per_entity
    }
}

/// Helper struct for executing the query from [`ViewContents`]
///
/// This restructures the [`QueryExpression`] into several sets that are
/// used to efficiently determine if we should continue the walk or switch
/// to a pure recursive evaluation.
struct QueryExpressionEvaluator<'a> {
    visualizers_per_entity: &'a IntMap<EntityPathHash, SmallVec<[ViewSystemIdentifier; 4]>>,
    entity_path_filter: &'a ResolvedEntityPathFilter,
    view_id: ViewId,
}

impl QueryExpressionEvaluator<'_> {
    fn add_entity_tree_to_data_results_recursive(
        &self,
        tree: &EntityTree,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
        num_matching_entities: &mut usize,
        num_visualized_entities: &mut usize,
    ) -> Option<DataResultHandle> {
        let filter_evaluation = self.entity_path_filter.evaluate(&tree.path);

        // Early-out optimization
        if !filter_evaluation.subtree_included {
            return None;
        }

        let entity_path = &tree.path;

        let matches_filter = filter_evaluation.matches;
        *num_matching_entities += matches_filter as usize;

        // If there are no visualizers at all, we can skip over some things.
        // Also, it's nice to inform the UI about how many entities we could show.
        let any_visualizers_available = matches_filter
            && self
                .visualizers_per_entity
                .get(&entity_path.hash())
                .is_some_and(|visualizer| !visualizer.is_empty());
        *num_visualized_entities += any_visualizers_available as usize;

        let children: SmallVec<[_; 4]> = tree
            .children
            .values()
            .filter_map(|subtree| {
                self.add_entity_tree_to_data_results_recursive(
                    subtree,
                    data_results,
                    num_matching_entities,
                    num_visualized_entities,
                )
            })
            .collect();

        // Ignore empty nodes.
        // Since we recurse downwards, this prunes any branches that don't have anything to contribute to the scene
        // and aren't directly included.
        let exact_included = filter_evaluation.matches_exactly;
        if exact_included || !children.is_empty() || any_visualizers_available {
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    any_visualizers_available,
                    visualizer_instructions: Default::default(), // Determined later during `update_overrides_recursive`.
                    tree_prefix_only: !matches_filter,
                    visible: true, // Determined later during `update_overrides_recursive`.
                    interactive: true, // Determined later during `update_overrides_recursive`.
                    override_base_path: ViewContents::base_override_path_for_entity(
                        self.view_id,
                        entity_path,
                    ),
                    query_range: QueryRange::default(), // Determined later during `update_overrides_recursive`.
                },
                children,
            }))
        } else {
            None
        }
    }
}

struct DataQueryPropertyResolver<'a> {
    view_query_range: &'a QueryRange,
    view_class: &'a dyn re_viewer_context::ViewClass,
    visualizable_entities_per_visualizer: &'a PerVisualizerTypeInViewClass<VisualizableEntities>,
    indicated_entities_per_visualizer: &'a PerVisualizerType<IndicatedEntities>,
}

impl DataQueryPropertyResolver<'_> {
    /// Recursively walk the [`DataResultTree`] and update each node.
    ///
    /// This will accumulate the recursive properties at each step down the tree, and then merge
    /// with individual overrides on each step.
    #[expect(clippy::too_many_arguments)]
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    fn update_overrides_recursive(
        &self,
        blueprint: &EntityDb,
        blueprint_query: &LatestAtQuery,
        active_timeline: Option<&Timeline>,
        query_result: &mut DataQueryResult,
        handle: DataResultHandle,
        default_query_range: &QueryRange,
        parent_visible: bool,
        parent_interactive: bool,
    ) {
        // This is called very frequently, don't put a profile scope here.

        let Some(node) = query_result.tree.data_results.get_mut(handle) else {
            return;
        };
        // Set defaults for time-range/visible/interactive.
        node.data_result.query_range = default_query_range.clone();
        node.data_result.visible = parent_visible;
        node.data_result.interactive = parent_interactive;

        let override_base_path = &node.data_result.override_base_path;

        // If the user has overridden the visualizers, update which visualizers are used.
        let id_component =
            blueprint_archetypes::ActiveVisualizers::descriptor_instruction_ids().component;
        let type_component =
            blueprint_archetypes::VisualizerInstruction::descriptor_visualizer_type().component;
        let component_map_component =
            blueprint_archetypes::VisualizerInstruction::descriptor_component_map().component;

        // Update visualizers from overrides.
        // So far, `visualizers` is set to the available visualizers.
        if node.data_result.any_visualizers_available {
            if let Some(visualizer_instruction_ids) = blueprint
                .latest_at(
                    blueprint_query,
                    &node.data_result.override_base_path,
                    [id_component],
                )
                .component_batch::<blueprint_components::VisualizerInstructionId>(id_component)
            {
                node.data_result.visualizer_instructions = visualizer_instruction_ids
                    .into_iter()
                    .map(|instruction_id| {
                        let visualizer_override_path = VisualizerInstruction::override_path_for(
                            &node.data_result.override_base_path,
                            &instruction_id,
                        );
                        let visualizer_type = blueprint
                            .latest_at(blueprint_query, &visualizer_override_path, [type_component])
                            .component_mono_quiet::<blueprint_components::VisualizerType>(
                                type_component,
                            )
                            .map_or_else(|| "No type specified".into(), |vt| vt.as_str().into());

                        VisualizerInstruction::new(
                            instruction_id,
                            visualizer_type,
                            &node.data_result.override_base_path,
                            // We're checking on stored component mappings later on since we also want to do so for otherwise
                            // heuristically generated visualizers.
                            // Practically, there should be no mappings stored if we're using heuristic visualizers,
                            // but we want to be consistent with overrides & mappings from the store both applying always.
                            VisualizerComponentMappings::default(),
                        )
                    })
                    .collect();
            } else {
                // Otherwise ask the `ViewClass` to choose.
                let recommended_visualizers = self.view_class.recommended_visualizers_for_entity(
                    &node.data_result.entity_path,
                    self.visualizable_entities_per_visualizer,
                    self.indicated_entities_per_visualizer,
                );
                node.data_result.visualizer_instructions = recommended_visualizers
                    .0
                    .into_iter()
                    .enumerate()
                    .map(|(index, (visualizer_type, component_mappings))| {
                        let id = VisualizerInstructionId::new_deterministic(
                            node.data_result.entity_path.hash64(),
                            index,
                        );
                        VisualizerInstruction::new(
                            id,
                            visualizer_type,
                            &node.data_result.override_base_path,
                            component_mappings,
                        )
                    })
                    .collect();
            }
        }

        // Map visualizer instruction IDs to data result handles for later use.
        query_result
            .tree
            .data_results_by_visualizer_instruction
            .extend(
                node.data_result
                    .visualizer_instructions
                    .iter()
                    .map(|vi| (vi.id, handle)),
            );

        // Gather "special" overrides directly on the base path (per entity).
        for component in blueprint
            .storage_engine()
            .store()
            .all_components_for_entity(override_base_path)
            .unwrap_or_default()
        {
            if let Some(component_data) = blueprint
                    .storage_engine()
                    .cache()
                    .latest_at(blueprint_query, override_base_path, [component])
                    .component_batch_raw(component)
                &&
                    // We regard empty overrides as non-existent. This is important because there is no other way of doing component-clears.
                     !component_data.is_empty()
            {
                // Handle special overrides:
                //
                // Visible time range override.
                if component
                    == blueprint_archetypes::VisibleTimeRanges::descriptor_ranges().component
                {
                    if let Some(active_timeline) = active_timeline
                        && let Ok(visible_time_ranges) =
                            blueprint_components::VisibleTimeRange::from_arrow(&component_data)
                        && let Some(time_range) = visible_time_ranges.iter().find(|range| {
                            range.timeline.as_str() == active_timeline.name().as_str()
                        })
                    {
                        node.data_result.query_range =
                            QueryRange::TimeRange(time_range.0.range.clone());
                    }
                }
                // Visible override.
                else if component
                    == blueprint_archetypes::EntityBehavior::descriptor_visible().component
                {
                    if let Some(visible_array) = component_data.as_boolean_opt() {
                        // We already checked for non-empty above, so this should be safe.
                        node.data_result.visible = visible_array.value(0);
                    }
                }
                // Interactive override.
                else if component
                    == blueprint_archetypes::EntityBehavior::descriptor_interactive().component
                    && let Some(interactive_array) = component_data.as_boolean_opt()
                {
                    // We already checked for non-empty above, so this should be safe.
                    node.data_result.interactive = interactive_array.value(0);
                }
            }
        }

        for instruction in &mut node.data_result.visualizer_instructions {
            // Gather "real" overrides on visualizer instruction specific path.
            // TODO(andreas): Why not keep the component data while we're here? Could speed up things a lot down the line.
            for component in blueprint
                .storage_engine()
                .store()
                .all_components_for_entity(&instruction.override_path)
                .unwrap_or_default()
            {
                if let Some(component_data) = blueprint
                        .storage_engine()
                        .cache()
                        .latest_at(blueprint_query, &instruction.override_path, [component])
                        .component_batch_raw(component) &&
                    // We regard empty overrides as non-existent. This is important because there is no other way of doing component-clears.
                     !component_data.is_empty()
                {
                    instruction.component_overrides.insert(component);
                }
            }

            // Gather component mappings. If we previously generated some via heuristic, we extend/overwrite those in favor of the ones stored in the store.
            if let Some(mappings_from_store) = blueprint
                .latest_at(
                    blueprint_query,
                    &instruction.override_path,
                    [component_map_component], // TODO(andreas): Should we batch more queries in general together here?
                )
                .component_batch::<blueprint_components::VisualizerComponentMapping>(
                    component_map_component,
                )
            {
                instruction
                    .component_mappings
                    .extend(mappings_from_store.into_iter().map(|mapping| {
                        (
                            mapping.target.as_str().into(),
                            re_viewer_context::VisualizerComponentSource::from_blueprint_mapping(
                                &mapping.0,
                            ),
                        )
                    }));
            }
        }

        let children = node.children.clone(); // Borrow-checker workaround.
        let visible = node.data_result.visible;
        let interactive = node.data_result.interactive;

        for child in children {
            self.update_overrides_recursive(
                blueprint,
                blueprint_query,
                active_timeline,
                query_result,
                child,
                default_query_range,
                visible,
                interactive,
            );
        }
    }

    /// Recursively walk the [`DataResultTree`] and update each node.
    pub fn update_overrides(
        &self,
        blueprint: &EntityDb,
        blueprint_query: &LatestAtQuery,
        active_timeline: Option<&Timeline>,
        query_result: &mut DataQueryResult,
    ) {
        re_tracing::profile_function!();

        if let Some(root) = query_result.tree.root_handle() {
            let default_query_range = self.view_query_range;
            let parent_visible = true;
            let parent_interactive = true;

            self.update_overrides_recursive(
                blueprint,
                blueprint_query,
                active_timeline,
                query_result,
                root,
                default_query_range,
                parent_visible,
                parent_interactive,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId};
    use re_entity_db::EntityDb;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreId, TimePoint, Timeline};
    use re_viewer_context::{
        Caches, StoreContext, ViewClassRegistry, VisualizableReason, blueprint_timeline,
    };

    use super::*;

    #[test]
    fn test_query_results() {
        let space_env = EntityPathSubs::empty();

        let mut recording = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));
        let blueprint = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Blueprint,
            "test_app",
        ));
        let view_class_identifier = "3D".into();

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let view_class_registry = ViewClassRegistry::default();

        // Set up a store DB with some entities
        for entity_path in ["parent", "parent/skipped/child1", "parent/skipped/child2"] {
            let row_id = RowId::new();
            let point = MyPoint::new(1.0, 2.0);
            let chunk = Chunk::builder(entity_path)
                .with_component_batch(
                    row_id,
                    timepoint.clone(),
                    (MyPoints::descriptor_points(), &[point] as _),
                )
                .build()
                .unwrap();

            recording.add_chunk(&Arc::new(chunk)).unwrap();
        }

        let mut visualizable_entities_for_visualizer_systems =
            PerVisualizerTypeInViewClass::<VisualizableEntities>::empty(view_class_identifier);

        visualizable_entities_for_visualizer_systems
            .per_visualizer
            .entry("Points3D".into())
            .or_insert_with(|| {
                VisualizableEntities(
                    [
                        (EntityPath::from("parent"), VisualizableReason::Always),
                        (
                            EntityPath::from("parent/skipped/child1"),
                            VisualizableReason::Always,
                        ),
                    ]
                    .into_iter()
                    .collect(),
                )
            });

        let ctx = StoreContext {
            blueprint: &blueprint,
            default_blueprint: None,
            recording: &recording,
            caches: &Caches::new(recording.store_id().clone()),
            should_enable_heuristics: false,
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
                    "/parent",
                    "/parent/skipped",
                    "/parent/skipped/child1", // Only child 1 has visualizers
                ],
            },
            Scenario {
                filter: "+ parent/skipped/**",
                outputs: vec![
                    "/**",
                    "/parent/**", // Only included because is a prefix
                    "/parent/skipped",
                    "/parent/skipped/child1", // Only child 1 has visualizers
                ],
            },
            Scenario {
                filter: r"+ parent
                          + parent/skipped/child2",
                outputs: vec![
                    "/**", // Trivial intermediate group -- could be collapsed
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
                    "/parent",
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
                    "/parent/**",             // Parent leaf has been excluded
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
                    "/parent",
                    "/parent/skipped",
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
            let view_id = ViewId::random();
            let contents = ViewContents::new(
                view_id,
                view_class_identifier,
                EntityPathFilter::parse_forgiving(filter).resolve_forgiving(&space_env),
            );

            let query_range = QueryRange::LatestAt;

            let query_result = contents.build_data_result_tree(
                &ctx,
                Some(&timeline_frame),
                &view_class_registry,
                &LatestAtQuery::latest(blueprint_timeline()),
                &query_range,
                &visualizable_entities_for_visualizer_systems,
                &PerVisualizerType::default(),
                &re_viewer_context::AppOptions::default(),
            );

            let mut visited = vec![];
            query_result.tree.visit(&mut |node| {
                let result = &node.data_result;
                if result.entity_path == EntityPath::root() {
                    visited.push("/**".to_owned());
                } else if result.tree_prefix_only {
                    visited.push(format!("{}/**", result.entity_path));
                    assert!(result.visualizer_instructions.is_empty());
                } else {
                    visited.push(result.entity_path.to_string());
                }
                true
            });

            assert_eq!(visited, outputs, "Scenario {i}, filter: {filter}");
        }
    }
}
