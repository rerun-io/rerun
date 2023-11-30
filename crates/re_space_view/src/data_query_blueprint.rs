use nohash_hasher::IntSet;
use once_cell::sync::Lazy;
use re_data_store::{
    EntityProperties, EntityPropertiesComponent, EntityPropertyMap, EntityTree, StoreDb,
};
use re_log_types::{DataRow, EntityPath, EntityPathExpr, RowId, TimePoint};
use re_viewer_context::{
    DataQueryId, DataQueryResult, DataResult, DataResultHandle, DataResultNode, DataResultTree,
    EntitiesPerSystem, EntitiesPerSystemPerClass, SpaceViewClassIdentifier, SpaceViewId,
    StoreContext, SystemCommand, SystemCommandSender as _, ViewerContext,
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
    pub space_view_class_identifier: SpaceViewClassIdentifier,
    pub expressions: QueryExpressions,
}

impl DataQueryBlueprint {
    pub fn is_equivalent(&self, other: &DataQueryBlueprint) -> bool {
        self.space_view_class_identifier
            .eq(&other.space_view_class_identifier)
            && self.expressions.eq(&other.expressions)
    }
}

impl DataQueryBlueprint {
    pub const INDIVIDUAL_OVERRIDES_PREFIX: &'static str = "individual_overrides";
    pub const RECURSIVE_OVERRIDES_PREFIX: &'static str = "recursive_overrides";

    pub fn new<'a>(
        space_view_class_identifier: SpaceViewClassIdentifier,
        queries_entities: impl Iterator<Item = &'a EntityPathExpr>,
    ) -> Self {
        Self {
            id: DataQueryId::random(),
            space_view_class_identifier,
            expressions: QueryExpressions {
                inclusions: queries_entities
                    .map(|exp| exp.to_string().into())
                    .collect::<Vec<_>>(),
                exclusions: vec![],
            },
        }
    }

    pub fn try_from_db(
        path: &EntityPath,
        blueprint_db: &StoreDb,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> Option<Self> {
        let expressions = blueprint_db
            .store()
            .query_timeless_component::<QueryExpressions>(path)
            .map(|c| c.value)?;

        let id = DataQueryId::from_entity_path(path);

        Some(Self {
            id,
            space_view_class_identifier,
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

    fn save_expressions(
        &self,
        ctx: &ViewerContext<'_>,
        inclusions: &[EntityPathExpr],
        exclusions: &[EntityPathExpr],
    ) {
        let timepoint = TimePoint::timeless();

        let expressions_component = QueryExpressions {
            inclusions: inclusions.iter().map(|s| s.to_string().into()).collect(),
            exclusions: exclusions.iter().map(|s| s.to_string().into()).collect(),
        };

        let row = DataRow::from_cells1_sized(
            RowId::random(),
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

    pub fn add_entity_exclusion(&self, ctx: &ViewerContext<'_>, expr: EntityPathExpr) {
        let mut edited = false;

        let mut inclusions: Vec<EntityPathExpr> = self.inclusions().collect();
        let mut exclusions: Vec<EntityPathExpr> = self.exclusions().collect();

        // This exclusion would cancel out any inclusions or exclusions that are descendants of it
        // so clean them up.
        if let Some(recursive_exclude) = expr.recursive_entity_path() {
            inclusions.retain(|inc_expr| {
                if inc_expr.entity_path().is_descendant_of(recursive_exclude) {
                    edited = true;
                    false
                } else {
                    true
                }
            });

            exclusions.retain(|exc_expr| {
                if exc_expr.entity_path().is_descendant_of(recursive_exclude) {
                    edited = true;
                    false
                } else {
                    true
                }
            });
        }

        inclusions.retain(|inc_expr| {
            if inc_expr == &expr {
                edited = true;
                false
            } else {
                true
            }
        });

        if !exclusions.iter().any(|exc_expr| exc_expr == &expr) {
            edited = true;
            exclusions.push(expr);
        }

        if edited {
            self.save_expressions(ctx, &inclusions, &exclusions);
        }
    }

    pub fn add_entity_inclusion(&self, ctx: &ViewerContext<'_>, expr: EntityPathExpr) {
        let mut edited = false;

        let mut inclusions: Vec<EntityPathExpr> = self.inclusions().collect();
        let mut exclusions: Vec<EntityPathExpr> = self.exclusions().collect();

        exclusions.retain(|exc_expr| {
            if exc_expr == &expr {
                edited = true;
                false
            } else {
                true
            }
        });

        if !inclusions.iter().any(|inc_expr| inc_expr == &expr) {
            edited = true;
            inclusions.push(expr);
        }

        if edited {
            self.save_expressions(ctx, &inclusions, &exclusions);
        }
    }

    pub fn clear_entity_expression(&self, ctx: &ViewerContext<'_>, expr: &EntityPathExpr) {
        let mut edited = false;

        let mut inclusions: Vec<EntityPathExpr> = self.inclusions().collect();
        let mut exclusions: Vec<EntityPathExpr> = self.exclusions().collect();

        exclusions.retain(|exc_expr| {
            if exc_expr == expr {
                edited = true;
                false
            } else {
                true
            }
        });

        inclusions.retain(|inc_expr| {
            if inc_expr == expr {
                edited = true;
                false
            } else {
                true
            }
        });

        if edited {
            self.save_expressions(ctx, &inclusions, &exclusions);
        }
    }

    pub fn inclusions(&self) -> impl Iterator<Item = EntityPathExpr> + '_ {
        self.expressions
            .inclusions
            .iter()
            .filter(|exp| !exp.as_str().is_empty())
            .map(|exp| EntityPathExpr::from(exp.as_str()))
    }

    pub fn exclusions(&self) -> impl Iterator<Item = EntityPathExpr> + '_ {
        self.expressions
            .exclusions
            .iter()
            .filter(|exp| !exp.as_str().is_empty())
            .map(|exp| EntityPathExpr::from(exp.as_str()))
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
            .get(&self.space_view_class_identifier)
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
    exact_inclusions: IntSet<EntityPath>,
    recursive_inclusions: IntSet<EntityPath>,
    exact_exclusions: IntSet<EntityPath>,
    recursive_exclusions: IntSet<EntityPath>,
    allowed_prefixes: IntSet<EntityPath>,
}

impl<'a> QueryExpressionEvaluator<'a> {
    fn new(
        blueprint: &'a DataQueryBlueprint,
        per_system_entity_list: &'a EntitiesPerSystem,
    ) -> Self {
        let inclusions: Vec<EntityPathExpr> = blueprint
            .expressions
            .inclusions
            .iter()
            .filter(|exp| !exp.as_str().is_empty())
            .map(|exp| EntityPathExpr::from(exp.as_str()))
            .collect();

        let exclusions: Vec<EntityPathExpr> = blueprint
            .expressions
            .exclusions
            .iter()
            .filter(|exp| !exp.as_str().is_empty())
            .map(|exp| EntityPathExpr::from(exp.as_str()))
            .collect();

        let exact_inclusions: IntSet<EntityPath> = inclusions
            .iter()
            .filter_map(|exp| exp.exact_entity_path().cloned())
            .collect();

        let recursive_inclusions = inclusions
            .iter()
            .filter_map(|exp| exp.recursive_entity_path().cloned())
            .collect();

        let exact_exclusions: IntSet<EntityPath> = exclusions
            .iter()
            .filter_map(|exp| exp.exact_entity_path().cloned())
            .collect();

        let recursive_exclusions: IntSet<EntityPath> = exclusions
            .iter()
            .filter_map(|exp| exp.recursive_entity_path().cloned())
            .collect();

        let allowed_prefixes = inclusions
            .iter()
            .flat_map(|exp| EntityPath::incremental_walk(None, exp.entity_path()))
            .collect();

        Self {
            blueprint,
            per_system_entity_list,
            exact_inclusions,
            recursive_inclusions,
            exact_exclusions,
            recursive_exclusions,
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
        // a pruned branch of the tree. Can come from either an explicit
        // recursive exclusion, or an implicit missing inclusion.
        // TODO(jleibs): If this space is disconnected, we should terminate here
        if self.recursive_exclusions.contains(&tree.path)
            || !(from_recursive || self.allowed_prefixes.contains(&tree.path))
        {
            return None;
        }

        let entity_path = tree.path.clone();

        // Pre-compute our matches
        let exact_include = self.exact_inclusions.contains(&entity_path);
        let recursive_include = self.recursive_inclusions.contains(&entity_path) || from_recursive;
        let exact_exclude = self.exact_exclusions.contains(&entity_path);
        let any_match = (exact_include || recursive_include) && !exact_exclude;

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

        let self_leaf = if !view_parts.is_empty() || exact_include {
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
                    direct_included: any_match,
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
                    recursive_include, // Once we have hit a recursive match, it's always propagated
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
                    direct_included: any_match,
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

        struct Scenario {
            inclusions: Vec<&'static str>,
            exclusions: Vec<&'static str>,
            outputs: Vec<&'static str>,
        }

        let scenarios: Vec<Scenario> = vec![
            Scenario {
                inclusions: vec!["/"],
                exclusions: vec![],
                outputs: vec![
                    "/",
                    "parent/",
                    "parent",
                    "parent/skipped/", // Not an exact match and not found in tree
                    "parent/skipped/child1", // Only child 1 has ViewParts
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped/"],
                exclusions: vec![],
                outputs: vec![
                    "/",
                    "parent/",               // Only included because is a prefix
                    "parent/skipped/",       // Not an exact match and not found in tree
                    "parent/skipped/child1", // Only child 1 has ViewParts
                ],
            },
            Scenario {
                inclusions: vec!["parent", "parent/skipped/child2"],
                exclusions: vec![],
                outputs: vec![
                    "/", // Trivial intermediate group -- could be collapsed
                    "parent/",
                    "parent",
                    "parent/skipped/", // Trivial intermediate group -- could be collapsed
                    "parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped", "parent/skipped/child2", "parent/"],
                exclusions: vec![],
                outputs: vec![
                    "/",
                    "parent/",
                    "parent",
                    "parent/skipped/",
                    "parent/skipped",        // Included because an exact match
                    "parent/skipped/child1", // Included because an exact match
                    "parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped", "parent/skipped/child2", "parent/"],
                exclusions: vec!["parent"],
                outputs: vec![
                    "/",
                    "parent/", // Parent leaf has been excluded
                    "parent/skipped/",
                    "parent/skipped",        // Included because an exact match
                    "parent/skipped/child1", // Included because an exact match
                    "parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/"],
                exclusions: vec!["parent/skipped/"],
                outputs: vec!["/", "parent"], // None of the children are hit since excluded
            },
            Scenario {
                inclusions: vec!["parent/", "parent/skipped/child2"],
                exclusions: vec!["parent/skipped/child1"],
                outputs: vec![
                    "/",
                    "parent/",
                    "parent",
                    "parent/skipped/",
                    "parent/skipped/child2", // No child1 since skipped.
                ],
            },
            Scenario {
                inclusions: vec!["not/found"],
                exclusions: vec![],
                // TODO(jleibs): Making this work requires merging the EntityTree walk with a minimal-coverage ExactMatchTree walk
                // not crucial for now until we expose a free-form UI for entering paths.
                // vec!["/", "not/", "not/found"]),
                outputs: vec![],
            },
        ];

        for Scenario {
            inclusions,
            exclusions,
            outputs,
        } in scenarios
        {
            let query = DataQueryBlueprint {
                id: DataQueryId::random(),
                space_view_class_identifier: "3D".into(),
                expressions: QueryExpressions {
                    inclusions: inclusions.into_iter().map(|s| s.into()).collect::<Vec<_>>(),
                    exclusions: exclusions.into_iter().map(|s| s.into()).collect::<Vec<_>>(),
                },
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
