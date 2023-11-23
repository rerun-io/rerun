use nohash_hasher::IntSet;
use once_cell::sync::Lazy;
use re_data_store::{
    EntityProperties, EntityPropertiesComponent, EntityPropertyMap, EntityTree, StoreDb,
};
use re_log_types::{EntityPath, EntityPathExpr};
use re_viewer_context::{
    DataQueryId, DataQueryResult, DataResult, DataResultHandle, DataResultNode, DataResultTree,
    EntitiesPerSystem, EntitiesPerSystemPerClass, SpaceViewClassName, SpaceViewId, StoreContext,
};
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{blueprint::QueryExpressions, DataQuery, EntityOverrides, PropertyResolver};

/// An implementation of [`DataQuery`] that is built from a collection of [`QueryExpressions`]
///
/// During execution it will walk an [`EntityTree`] and return a [`DataResultTree`]
/// containing any entities that match the input list of [`EntityPathExpr`]s.
///
/// Any exact expressions are included in the [`DataResultTree`] regardless of whether or not they are found
/// this allows UI to show a "no data" message for entities that are being explicitly looked for but
/// not found.
///
/// The results of recursive expressions are only included if they are found within the [`EntityTree`]
/// and for which there is a valid `ViewPart` system. This keeps recursive expressions from incorrectly
/// picking up irrelevant data within the tree.
#[derive(Clone, PartialEq, Eq)]
pub struct DataQueryBlueprint {
    pub id: DataQueryId,
    pub space_view_class_name: SpaceViewClassName,
    pub expressions: QueryExpressions,
}

impl DataQueryBlueprint {
    pub const INDIVIDUAL_OVERRIDES_PREFIX: &str = "individual_overrides";
    pub const RECURSIVE_OVERRIDES_PREFIX: &str = "recursive_overrides";

    pub fn auto(space_view_class_name: SpaceViewClassName, space_path: &EntityPath) -> Self {
        Self {
            id: DataQueryId::random(),
            space_view_class_name,
            expressions: vec![EntityPathExpr::Recursive(space_path.clone())
                .to_string()
                .into()]
            .into(),
        }
    }

    pub fn try_from_db(
        path: &EntityPath,
        blueprint_db: &StoreDb,
        space_view_class_name: SpaceViewClassName,
    ) -> Option<Self> {
        let expressions = blueprint_db
            .store()
            .query_timeless_component::<QueryExpressions>(path)
            .map(|c| c.value)?;

        let id = DataQueryId::from_entity_path(path);

        Some(Self {
            id,
            space_view_class_name,
            expressions,
        })
    }

    pub fn build_resolver<'a>(
        &self,
        container: SpaceViewId,
        auto_properties: &'a EntityPropertyMap,
    ) -> DataQueryPropertyResolver<'a> {
        DataQueryPropertyResolver {
            auto_properties,
            default_stack: vec![container.as_entity_path(), self.id.as_entity_path()],
            individual_override_root: self
                .id
                .as_entity_path()
                .join(&Self::INDIVIDUAL_OVERRIDES_PREFIX.into()),
            recursive_override_root: self
                .id
                .as_entity_path()
                .join(&Self::RECURSIVE_OVERRIDES_PREFIX.into()),
        }
    }
}

impl DataQuery for DataQueryBlueprint {
    fn execute_query(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &re_viewer_context::StoreContext<'_>,
        entities_per_system_per_class: &EntitiesPerSystemPerClass,
    ) -> DataQueryResult {
        re_tracing::profile_function!();

        static EMPTY_ENTITY_LIST: Lazy<EntitiesPerSystem> = Lazy::new(Default::default);

        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();

        let overrides = property_resolver.resolve_entity_overrides(ctx);

        let per_system_entity_list = entities_per_system_per_class
            .get(&self.space_view_class_name)
            .unwrap_or(&EMPTY_ENTITY_LIST);

        let executor = QueryExpressionEvaluator::new(self, per_system_entity_list);

        let root_handle = ctx.recording.and_then(|store| {
            executor.add_entity_tree_to_data_results_recursive(
                &store.entity_db().tree,
                &overrides,
                &overrides.root,
                &mut data_results,
                false,
            )
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
    blueprint: &'a DataQueryBlueprint,
    per_system_entity_list: &'a EntitiesPerSystem,
    exact_matches: IntSet<EntityPath>,
    recursive_matches: IntSet<EntityPath>,
    allowed_prefixes: IntSet<EntityPath>,
}

impl<'a> QueryExpressionEvaluator<'a> {
    fn new(
        blueprint: &'a DataQueryBlueprint,
        per_system_entity_list: &'a EntitiesPerSystem,
    ) -> Self {
        let expressions: Vec<EntityPathExpr> = blueprint
            .expressions
            .expressions
            .iter()
            .map(|exp| EntityPathExpr::from(exp.as_str()))
            .collect();

        let exact_matches: IntSet<EntityPath> = expressions
            .iter()
            .filter_map(|exp| exp.exact_entity_path().cloned())
            .collect();

        let recursive_matches = expressions
            .iter()
            .filter_map(|exp| exp.recursive_entity_path().cloned())
            .collect();

        let allowed_prefixes = expressions
            .iter()
            .flat_map(|exp| EntityPath::incremental_walk(None, exp.entity_path()))
            .collect();

        Self {
            blueprint,
            per_system_entity_list,
            exact_matches,
            recursive_matches,
            allowed_prefixes,
        }
    }

    fn add_entity_tree_to_data_results_recursive(
        &self,
        tree: &EntityTree,
        overrides: &EntityOverrides,
        inherited: &EntityProperties,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
        from_recursive: bool,
    ) -> Option<DataResultHandle> {
        // If we hit a prefix that is not allowed, we terminate. This is
        // a pruned branch of the tree.
        // TODO(jleibs): If this space is disconnected, we should terminate here
        if !(from_recursive || self.allowed_prefixes.contains(&tree.path)) {
            return None;
        }

        let entity_path = tree.path.clone();

        // Pre-compute our matches
        let exact_match = self.exact_matches.contains(&entity_path);
        let recursive_match = self.recursive_matches.contains(&entity_path) || from_recursive;
        let any_match = exact_match || recursive_match;

        // Only populate view_parts if this is a match
        // Note that allowed prefixes that aren't matches can still create groups
        let view_parts: SmallVec<_> = if any_match {
            self.per_system_entity_list
                .iter()
                .filter_map(|(part, ents)| {
                    if ents.contains(&entity_path) {
                        Some(*part)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Default::default()
        };

        let mut resolved_properties = inherited.clone();

        if let Some(props) = overrides.group.get_opt(&entity_path) {
            resolved_properties = resolved_properties.with_child(props);
        }

        let base_entity_path = self.blueprint.id.as_entity_path().clone();

        let individual_override_path = base_entity_path
            .join(&DataQueryBlueprint::INDIVIDUAL_OVERRIDES_PREFIX.into())
            .join(&entity_path);
        let recursive_override_path = base_entity_path
            .join(&DataQueryBlueprint::RECURSIVE_OVERRIDES_PREFIX.into())
            .join(&entity_path);

        let self_leaf = if !view_parts.is_empty() || exact_match {
            let individual_props = overrides.individual.get_opt(&entity_path);
            let mut leaf_resolved_properties = resolved_properties.clone();

            if let Some(props) = individual_props {
                leaf_resolved_properties = leaf_resolved_properties.with_child(props);
            }
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    view_parts,
                    is_group: false,
                    individual_properties: overrides.individual.get_opt(&entity_path).cloned(),
                    resolved_properties: leaf_resolved_properties,
                    override_path: individual_override_path,
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
                self.add_entity_tree_to_data_results_recursive(
                    subtree,
                    overrides,
                    &resolved_properties,
                    data_results,
                    recursive_match, // Once we have hit a recursive match, it's always propagated
                )
            }))
            .collect();

        // If the only child is the self-leaf, then we don't need to create a group
        if children.is_empty() || children.len() == 1 && self_leaf.is_some() {
            self_leaf
        } else {
            // The 'individual' properties of a group are the group overrides
            let individual_properties = overrides.group.get_opt(&entity_path).cloned();
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path,
                    view_parts: Default::default(),
                    is_group: true,
                    individual_properties,
                    resolved_properties,
                    override_path: recursive_override_path,
                },
                children,
            }))
        }
    }
}

pub struct DataQueryPropertyResolver<'a> {
    auto_properties: &'a EntityPropertyMap,
    default_stack: Vec<EntityPath>,
    individual_override_root: EntityPath,
    recursive_override_root: EntityPath,
}

impl DataQueryPropertyResolver<'_> {
    fn resolve_entity_overrides_for_path(
        &self,
        ctx: &StoreContext<'_>,
        props_path: &EntityPath,
    ) -> EntityPropertyMap {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut prop_map = self.auto_properties.clone();

        if let Some(tree) = blueprint.entity_db().tree.subtree(props_path) {
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

impl<'a> PropertyResolver for DataQueryPropertyResolver<'a> {
    /// Helper function to lookup the properties for a given entity path.
    ///
    /// We start with the auto properties for the `SpaceView` as the base layer and
    /// then incrementally override from there.
    fn resolve_entity_overrides(&self, ctx: &StoreContext<'_>) -> EntityOverrides {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut root: EntityProperties = Default::default();
        for prefix in &self.default_stack {
            if let Some(overrides) = ctx
                .blueprint
                .store()
                .query_timeless_component::<EntityPropertiesComponent>(prefix)
            {
                root = root.with_child(&overrides.value.props);
            }
        }

        let mut individual = self.auto_properties.clone();

        if let Some(tree) = blueprint
            .entity_db()
            .tree
            .subtree(&self.individual_override_root)
        {
            tree.visit_children_recursively(&mut |path: &EntityPath| {
                if let Some(props) = blueprint
                    .store()
                    .query_timeless_component::<EntityPropertiesComponent>(path)
                {
                    let overridden_path = EntityPath::from(
                        &path.as_slice()[self.individual_override_root.len()..path.len()],
                    );
                    individual.update(overridden_path, props.value.props);
                }
            });
        }

        EntityOverrides {
            root,
            individual: self.resolve_entity_overrides_for_path(ctx, &self.individual_override_root),
            group: self.resolve_entity_overrides_for_path(ctx, &self.recursive_override_root),
        }
    }
}

#[cfg(feature = "testing")]
#[cfg(test)]
mod tests {
    use re_data_store::StoreDb;
    use re_log_types::{example_components::MyPoint, DataRow, RowId, StoreId, TimePoint, Timeline};
    use re_viewer_context::StoreContext;

    use super::*;

    struct StaticPropertyResolver {
        overrides: EntityPropertyMap,
    }

    impl PropertyResolver for StaticPropertyResolver {
        fn resolve_entity_overrides(&self, _ctx: &StoreContext<'_>) -> EntityOverrides {
            EntityOverrides {
                root: Default::default(),
                individual: self.overrides.clone(),
                group: self.overrides.clone(),
            }
        }
    }

    #[test]
    fn test_query_results() {
        let mut recording = StoreDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        let blueprint = StoreDb::new(StoreId::random(re_log_types::StoreKind::Blueprint));

        let overrides = EntityPropertyMap::default();
        let resolver = StaticPropertyResolver { overrides };

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);

        // Set up a store DB with some entities
        for entity_path in ["parent", "parent/skipped/child1", "parent/skipped/child2"] {
            let row_id = RowId::random();
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

        let ctx = StoreContext {
            blueprint: &blueprint,
            recording: Some(&recording),
            all_recordings: vec![],
        };

        let scenarios: Vec<(Vec<&str>, Vec<&str>)> = vec![
            (
                vec!["/"],
                vec![
                    "/",
                    "parent/",
                    "parent",
                    "parent/skipped/", // Not an exact match and not found in tree
                    "parent/skipped/child1", // Only child 1 has ViewParts
                ],
            ),
            (
                vec!["parent/skipped/"],
                vec![
                    "/",
                    "parent/",               // Only included because is a prefix
                    "parent/skipped/",       // Not an exact match and not found in tree
                    "parent/skipped/child1", // Only child 1 has ViewParts
                ],
            ),
            (
                vec!["parent", "parent/skipped/child2"],
                vec![
                    "/", // Trivial intermediate group -- could be collapsed
                    "parent/",
                    "parent",
                    "parent/skipped/", // Trivial intermediate group -- could be collapsed
                    "parent/skipped/child2",
                ],
            ),
            (
                vec!["parent/skipped", "parent/skipped/child2", "parent/"],
                vec![
                    "/",
                    "parent/",
                    "parent",
                    "parent/skipped/",
                    "parent/skipped",        // Included because an exact match
                    "parent/skipped/child1", // Included because an exact match
                    "parent/skipped/child2",
                ],
            ),
            (
                vec!["not/found"],
                // TODO(jleibs): Making this work requires merging the EntityTree walk with a minimal-coverage ExactMatchTree walk
                // not crucial for now until we expose a free-form UI for entering paths.
                // vec!["/", "not/", "not/found"]),
                vec![],
            ),
        ];

        for (input, outputs) in scenarios {
            let query = DataQueryBlueprint {
                id: DataQueryId::random(),
                space_view_class_name: "3D".into(),
                expressions: input
                    .into_iter()
                    .map(|s| s.into())
                    .collect::<Vec<_>>()
                    .into(),
            };

            let query_result = query.execute_query(&resolver, &ctx, &entities_per_system_per_class);

            let mut visited = vec![];
            query_result.tree.visit(&mut |handle| {
                let result = query_result.tree.lookup_result(handle).unwrap();
                if result.is_group && result.entity_path != EntityPath::root() {
                    visited.push(format!("{}/", result.entity_path));
                } else {
                    visited.push(result.entity_path.to_string());
                }
            });

            assert_eq!(visited, outputs);
        }
    }
}
