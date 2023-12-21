use nohash_hasher::IntSet;
use re_data_store::{
    EntityProperties, EntityPropertiesComponent, EntityPropertyMap, EntityTree, StoreDb,
};
use re_log_types::{DataRow, EntityPath, EntityPathExpr, RowId, TimePoint};
use re_tracing::profile_scope;
use re_types_core::archetypes::Clear;
use re_viewer_context::{
    DataQueryId, DataQueryResult, DataResult, DataResultHandle, DataResultNode, DataResultTree,
    EntitiesPerSystem, PropertyOverrides, SpaceViewClassIdentifier, SpaceViewId, StoreContext,
    SystemCommand, SystemCommandSender as _, ViewerContext,
};
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{
    blueprint::components::QueryExpressions, DataQuery, EntityOverrides, PropertyResolver,
};

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
///
/// Note: [`DataQueryBlueprint`] doesn't implement Clone because it stores an internal
/// uuid used for identifying the path of its data in the blueprint store. It's ambiguous
/// whether the intent is for a clone to write to the same place.
///
/// If you want a new space view otherwise identical to an existing one, use
/// [`DataQueryBlueprint::duplicate`].
#[derive(PartialEq, Eq)]
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
    /// Creates a new [`DataQueryBlueprint`].
    ///
    /// This [`DataQueryBlueprint`] is ephemeral. It must be saved by calling
    /// `save_to_blueprint_store` on the enclosing `SpaceViewBlueprint`.
    pub fn new(
        space_view_class_identifier: SpaceViewClassIdentifier,
        queries_entities: impl Iterator<Item = EntityPathExpr>,
    ) -> Self {
        Self {
            id: DataQueryId::random(),
            space_view_class_identifier,
            expressions: QueryExpressions::new(queries_entities, std::iter::empty()),
        }
    }

    /// Attempt to load a [`DataQueryBlueprint`] from the blueprint store.
    pub fn try_from_db(
        id: DataQueryId,
        blueprint_db: &StoreDb,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> Option<Self> {
        let expressions = blueprint_db
            .store()
            .query_timeless_component::<QueryExpressions>(&id.as_entity_path())
            .map(|c| c.value)?;

        Some(Self {
            id,
            space_view_class_identifier,
            expressions,
        })
    }

    /// Persist the entire [`DataQueryBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`DataQueryBlueprint`] was created with [`Self::new`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        ctx.save_blueprint_component(&self.id.as_entity_path(), self.expressions.clone());
    }

    /// Creates a new [`DataQueryBlueprint`] with a the same contents, but a different [`DataQueryId`]
    pub fn duplicate(&self) -> Self {
        Self {
            id: DataQueryId::random(),
            space_view_class_identifier: self.space_view_class_identifier,
            expressions: self.expressions.clone(),
        }
    }

    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        let clear = Clear::recursive();
        ctx.save_blueprint_component(&self.id.as_entity_path(), clear.is_recursive);
    }

    pub fn build_resolver<'a>(
        &self,
        container: SpaceViewId,
        auto_properties: &'a EntityPropertyMap,
    ) -> DataQueryPropertyResolver<'a> {
        let base_override_root = self.id.as_entity_path().clone();
        let individual_override_root =
            base_override_root.join(&DataResult::INDIVIDUAL_OVERRIDES_PREFIX.into());
        let recursive_override_root =
            base_override_root.join(&DataResult::RECURSIVE_OVERRIDES_PREFIX.into());
        DataQueryPropertyResolver {
            auto_properties,
            default_stack: vec![container.as_entity_path(), self.id.as_entity_path()],
            individual_override_root,
            recursive_override_root,
        }
    }

    fn save_expressions(
        &self,
        ctx: &ViewerContext<'_>,
        inclusions: impl Iterator<Item = EntityPathExpr>,
        exclusions: impl Iterator<Item = EntityPathExpr>,
    ) {
        let timepoint = TimePoint::timeless();

        let expressions_component = QueryExpressions::new(inclusions, exclusions);

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
            self.save_expressions(ctx, inclusions.into_iter(), exclusions.into_iter());
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
            self.save_expressions(ctx, inclusions.into_iter(), exclusions.into_iter());
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
            self.save_expressions(ctx, inclusions.into_iter(), exclusions.into_iter());
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
        ctx: &re_viewer_context::StoreContext<'_>,
        entities_per_system: &EntitiesPerSystem,
    ) -> DataQueryResult {
        re_tracing::profile_function!();

        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();

        let executor = QueryExpressionEvaluator::new(self, entities_per_system);

        let root_handle = {
            profile_scope!("run queries");
            ctx.recording.and_then(|store| {
                executor.add_entity_tree_to_data_results_recursive(
                    store.tree(),
                    &mut data_results,
                    false,
                )
            })
        };

        {
            profile_scope!("return results");

            DataQueryResult {
                id: self.id,
                tree: DataResultTree::new(data_results, root_handle),
            }
        }
    }
}

/// Helper struct for executing the query from [`DataQueryBlueprint`]
///
/// This restructures the [`QueryExpressions`] into several sets that are
/// used to efficiently determine if we should continue the walk or switch
/// to a pure recursive evaluation.
struct QueryExpressionEvaluator<'a> {
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
        re_tracing::profile_function!();
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

        let entity_path = &tree.path;

        // Pre-compute our matches
        let exact_include = self.exact_inclusions.contains(entity_path);
        let recursive_include = self.recursive_inclusions.contains(entity_path) || from_recursive;
        let exact_exclude = self.exact_exclusions.contains(entity_path);
        let any_match = (exact_include || recursive_include) && !exact_exclude;

        // Only populate view_parts if this is a match
        // Note that allowed prefixes that aren't matches can still create groups
        let view_parts: SmallVec<_> = if any_match {
            self.per_system_entity_list
                .iter()
                .filter_map(|(part, ents)| {
                    if ents.contains(entity_path) {
                        Some(*part)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Default::default()
        };

        let self_leaf = if !view_parts.is_empty() || exact_include {
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    view_parts,
                    is_group: false,
                    direct_included: any_match,
                    property_overrides: None,
                },
                children: Default::default(),
            }))
        } else {
            None
        };

        let maybe_self_iter = {
            if let Some(self_leaf) = self_leaf {
                itertools::Either::Left(std::iter::once(self_leaf))
            } else {
                itertools::Either::Right(std::iter::empty())
            }
        };

        let interesting_children: Vec<_> = {
            if recursive_include {
                tree.children
                    .values()
                    .filter(|subtree| {
                        !(self.recursive_exclusions.contains(&subtree.path)
                            || !(recursive_include
                                || self.allowed_prefixes.contains(&subtree.path)))
                    })
                    .collect()
            } else {
                self.allowed_prefixes
                    .iter()
                    .filter(|prefix| prefix.is_child_of(entity_path))
                    .filter_map(|prefix| prefix.last().and_then(|part| tree.children.get(part)))
                    .collect()
            }
        };

        let children: SmallVec<_> = {
            maybe_self_iter
                .chain(interesting_children.iter().filter_map(|subtree| {
                    self.add_entity_tree_to_data_results_recursive(
                        subtree,
                        data_results,
                        recursive_include, // Once we have hit a recursive match, it's always propagated
                    )
                }))
                .collect()
        };

        // If the only child is the self-leaf, then we don't need to create a group
        if children.is_empty() || children.len() == 1 && self_leaf.is_some() {
            self_leaf
        } else {
            // The 'individual' properties of a group are the group overrides
            Some(data_results.insert(DataResultNode {
                data_result: DataResult {
                    entity_path: entity_path.clone(),
                    view_parts: Default::default(),
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
    auto_properties: &'a EntityPropertyMap,
    default_stack: Vec<EntityPath>,
    individual_override_root: EntityPath,
    recursive_override_root: EntityPath,
}

impl DataQueryPropertyResolver<'_> {
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
                root = root.with_child(&overrides.value.0);
            }
        }

        let mut individual = self.auto_properties.clone();

        if let Some(tree) = blueprint.tree().subtree(&self.individual_override_root) {
            tree.visit_children_recursively(&mut |path: &EntityPath| {
                if let Some(props) = blueprint
                    .store()
                    .query_timeless_component::<EntityPropertiesComponent>(path)
                {
                    let overridden_path = EntityPath::from(
                        &path.as_slice()[self.individual_override_root.len()..path.len()],
                    );
                    individual.update(overridden_path, props.value.0);
                }
            });
        }

        EntityOverrides {
            root,
            individual: self.resolve_entity_overrides_for_path(ctx, &self.individual_override_root),
            group: self.resolve_entity_overrides_for_path(ctx, &self.recursive_override_root),
        }
    }

    fn resolve_entity_overrides_for_path(
        &self,
        ctx: &StoreContext<'_>,
        props_path: &EntityPath,
    ) -> EntityPropertyMap {
        re_tracing::profile_function!();
        let blueprint = ctx.blueprint;

        let mut prop_map = self.auto_properties.clone();

        if let Some(tree) = blueprint.tree().subtree(props_path) {
            tree.visit_children_recursively(&mut |path: &EntityPath| {
                if let Some(props) = blueprint
                    .store()
                    .query_timeless_component_quiet::<EntityPropertiesComponent>(path)
                {
                    let overridden_path =
                        EntityPath::from(&path.as_slice()[props_path.len()..path.len()]);
                    prop_map.update(overridden_path, props.value.0);
                }
            });
        }
        prop_map
    }

    fn update_overrides_recursive(
        &self,
        query_result: &mut DataQueryResult,
        entity_overrides: &EntityOverrides,
        accumulated: &EntityProperties,
        handle: DataResultHandle,
    ) {
        if let Some((child_handles, accumulated)) =
            query_result.tree.lookup_node_mut(handle).and_then(|node| {
                if node.data_result.is_group {
                    let overridden_properties = entity_overrides
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
                        override_path: self
                            .recursive_override_root
                            .join(&node.data_result.entity_path),
                    });

                    Some((node.children.clone(), accumulated_properties))
                } else {
                    let overridden_properties = entity_overrides
                        .individual
                        .get_opt(&node.data_result.entity_path);

                    let accumulated_properties = if let Some(overridden) = overridden_properties {
                        accumulated.with_child(overridden)
                    } else {
                        accumulated.clone()
                    };

                    node.data_result.property_overrides = Some(PropertyOverrides {
                        individual_properties: overridden_properties.cloned(),
                        accumulated_properties: accumulated_properties.clone(),
                        override_path: self
                            .individual_override_root
                            .join(&node.data_result.entity_path),
                    });

                    None
                }
            })
        {
            for child in child_handles {
                self.update_overrides_recursive(
                    query_result,
                    entity_overrides,
                    &accumulated,
                    child,
                );
            }
        }
    }
}

impl<'a> PropertyResolver for DataQueryPropertyResolver<'a> {
    fn update_overrides(&self, ctx: &StoreContext<'_>, query_result: &mut DataQueryResult) {
        let entity_overrides = self.resolve_entity_overrides(ctx);

        if let Some(root) = query_result.tree.root_handle() {
            self.update_overrides_recursive(
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
    use re_data_store::StoreDb;
    use re_log_types::{example_components::MyPoint, DataRow, RowId, StoreId, TimePoint, Timeline};
    use re_viewer_context::StoreContext;

    use super::*;

    #[test]
    fn test_query_results() {
        let mut recording = StoreDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        let blueprint = StoreDb::new(StoreId::random(re_log_types::StoreKind::Blueprint));

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

        let mut entities_per_system = EntitiesPerSystem::default();

        entities_per_system
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
                    "/parent/",
                    "/parent",
                    "/parent/skipped/", // Not an exact match and not found in tree
                    "/parent/skipped/child1", // Only child 1 has ViewParts
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped/"],
                exclusions: vec![],
                outputs: vec![
                    "/",
                    "/parent/",               // Only included because is a prefix
                    "/parent/skipped/",       // Not an exact match and not found in tree
                    "/parent/skipped/child1", // Only child 1 has ViewParts
                ],
            },
            Scenario {
                inclusions: vec!["parent", "parent/skipped/child2"],
                exclusions: vec![],
                outputs: vec![
                    "/", // Trivial intermediate group -- could be collapsed
                    "/parent/",
                    "/parent",
                    "/parent/skipped/", // Trivial intermediate group -- could be collapsed
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped", "parent/skipped/child2", "parent/"],
                exclusions: vec![],
                outputs: vec![
                    "/",
                    "/parent/",
                    "/parent",
                    "/parent/skipped/",
                    "/parent/skipped",        // Included because an exact match
                    "/parent/skipped/child1", // Included because an exact match
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/skipped", "parent/skipped/child2", "parent/"],
                exclusions: vec!["parent"],
                outputs: vec![
                    "/",
                    "/parent/", // Parent leaf has been excluded
                    "/parent/skipped/",
                    "/parent/skipped",        // Included because an exact match
                    "/parent/skipped/child1", // Included because an exact match
                    "/parent/skipped/child2",
                ],
            },
            Scenario {
                inclusions: vec!["parent/"],
                exclusions: vec!["parent/skipped/"],
                outputs: vec!["/", "/parent"], // None of the children are hit since excluded
            },
            Scenario {
                inclusions: vec!["parent/", "parent/skipped/child2"],
                exclusions: vec!["parent/skipped/child1"],
                outputs: vec![
                    "/",
                    "/parent/",
                    "/parent",
                    "/parent/skipped/",
                    "/parent/skipped/child2", // No child1 since skipped.
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
                expressions: QueryExpressions::new(
                    inclusions.into_iter().map(EntityPathExpr::from),
                    exclusions.into_iter().map(EntityPathExpr::from),
                ),
            };

            let query_result = query.execute_query(&ctx, &entities_per_system);

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
