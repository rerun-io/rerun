use std::collections::BTreeSet;

use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap};
use re_viewer_context::{DataResult, ViewerContext};
use slotmap::SlotMap;

use crate::{DataBlueprintGroup, SpaceViewContents};

slotmap::new_key_type! {
    /// Identifier for a [`DataResultNode`]
    pub struct DataResultHandle;
}

/// A hierarchical tree of [`DataResult`]s
pub struct DataResultTree {
    data_results: SlotMap<DataResultHandle, DataResultNode>,
    root_handle: DataResultHandle,
}

impl DataResultTree {
    /// Depth-first traversal of the tree, calling `visitor` on each result.
    pub fn visit(&self, visitor: &mut impl FnMut(DataResultHandle)) {
        self.visit_recursive(self.root_handle, visitor);
    }

    /// Look up a node in the tree based on its handle.
    pub fn lookup(&self, handle: DataResultHandle) -> Option<&DataResult> {
        self.data_results.get(handle).map(|node| &node.data_result)
    }

    fn visit_recursive(
        &self,
        handle: DataResultHandle,
        visitor: &mut impl FnMut(DataResultHandle),
    ) {
        if let Some(result) = self.data_results.get(handle) {
            visitor(handle);

            for child in &result.children {
                self.visit_recursive(*child, visitor);
            }
        }
    }
}

/// A single node in the [`DataResultTree`]
#[derive(Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    children: BTreeSet<DataResultHandle>,
}

/// Trait for resolving properties needed by most implementations of [`DataQuery`]
///
/// The `SpaceViewBlueprint` is the only thing that likely implements this today
/// but we use a trait here so we don't have to pick up a full dependency on `re_viewport`.
pub trait PropertyResolver {
    fn resolve_entity_overrides(&self, ctx: &ViewerContext<'_>) -> EntityPropertyMap;
    fn resolve_root_override(&self, ctx: &ViewerContext<'_>) -> EntityProperties;
}

/// The common trait implemented for data queries
///
/// Both interfaces return [`DataResult`]s, which are self-contained description of the data
/// to be added to a `SpaceView` including both the [`EntityPath`] and context for any overrides.
pub trait DataQuery {
    /// Execute a full query, returning a `DataResultTree` containing all results.
    ///
    /// `auto_properties` is a map containing any heuristic-derived auto properties for the given `SpaceView`.
    ///
    /// This is used when building up the contents for a `SpaceView`.
    fn execute_query(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &ViewerContext<'_>,
    ) -> DataResultTree;

    /// Find a single [`DataResult`] within the context of the query.
    ///
    /// `auto_properties` is a map containing any heuristic-derived auto properties for the given `SpaceView`.
    ///
    /// This is used when finding the result for a single entity such as in
    /// a selection panel.
    fn resolve(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &ViewerContext<'_>,
        entity_path: &EntityPath,
    ) -> DataResult;
}

/// Helper function to iterate over all incremental [`EntityPath`]s from start to end, NOT including start itself.
///
/// For example `incremental_walk("foo", "foo/bar/baz")` returns: `["foo/bar", "foo/bar/baz"]`
fn incremental_walk<'a>(
    start: Option<&'_ EntityPath>,
    end: &'a EntityPath,
) -> impl Iterator<Item = EntityPath> + 'a {
    re_tracing::profile_function!();
    if start.map_or(true, |start| end.is_descendant_of(start)) {
        let first_ind = start.map_or(0, |start| start.len() + 1);
        let parts = end.as_slice();
        itertools::Either::Left((first_ind..=end.len()).map(|i| EntityPath::from(&parts[0..i])))
    } else {
        itertools::Either::Right(std::iter::empty())
    }
}

// ----------------------------------------------------------------------------
// Implement the `DataQuery` interface for `SpaceViewContents`

impl DataQuery for SpaceViewContents {
    fn execute_query(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &ViewerContext<'_>,
    ) -> DataResultTree {
        re_tracing::profile_function!();
        let root_override = property_resolver.resolve_root_override(ctx);
        let entity_overrides = property_resolver.resolve_entity_overrides(ctx);
        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();
        let root_handle = self.root_group().add_to_data_results_recursive(
            self,
            &entity_overrides,
            None,
            &root_override,
            &mut data_results,
        );
        DataResultTree {
            data_results,
            root_handle,
        }
    }

    fn resolve(
        &self,
        property_resolver: &impl PropertyResolver,
        ctx: &ViewerContext<'_>,
        entity_path: &EntityPath,
    ) -> DataResult {
        re_tracing::profile_function!();
        let entity_overrides = property_resolver.resolve_entity_overrides(ctx);

        let view_parts = self
            .per_system_entities()
            .iter()
            .filter_map(|(part, ents)| {
                if ents.contains(entity_path) {
                    Some(*part)
                } else {
                    None
                }
            })
            .collect();

        let mut resolved_properties = property_resolver.resolve_root_override(ctx);
        for prefix in incremental_walk(None, entity_path) {
            resolved_properties = resolved_properties.with_child(&entity_overrides.get(&prefix));
        }

        DataResult {
            entity_path: entity_path.clone(),
            view_parts,
            resolved_properties,
            individual_properties: entity_overrides.get_opt(entity_path).cloned(),
            override_path: self
                .entity_path()
                .join(&SpaceViewContents::PROPERTIES_PREFIX.into())
                .join(entity_path),
        }
    }
}

impl DataBlueprintGroup {
    /// This recursively walks a `DataBlueprintGroup` and adds every entity / group to the tree.
    ///
    /// Properties are resolved hierarchically from an [`EntityPropertyMap`] containing all the
    /// overrides. As we walk down the tree.
    fn add_to_data_results_recursive(
        &self,
        contents: &SpaceViewContents,
        overrides: &EntityPropertyMap,
        inherited_base: Option<&EntityPath>,
        inherited: &EntityProperties,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
    ) -> DataResultHandle {
        let group_path = self.group_path.clone();

        // TODO(jleibs): This remapping isn't great when a view has a bunch of entity-types.
        let group_view_parts = contents.view_parts_for_entity_path(&group_path);

        let mut group_resolved_properties = inherited.clone();

        for prefix in incremental_walk(inherited_base, &group_path) {
            if let Some(props) = overrides.get_opt(&prefix) {
                group_resolved_properties = group_resolved_properties.with_child(props);
            }
        }

        let base_entity_path = contents.entity_path();
        let props_path = EntityPath::from(SpaceViewContents::PROPERTIES_PREFIX);

        let group_override_path = base_entity_path.join(&props_path).join(&group_path);

        // First build up the direct children
        let mut children: BTreeSet<DataResultHandle> = self
            .entities
            .iter()
            .filter(|entity| group_path != **entity)
            .cloned()
            .map(|entity_path| {
                let view_parts = contents.view_parts_for_entity_path(&entity_path);

                let mut resolved_properties = group_resolved_properties.clone();

                for prefix in incremental_walk(inherited_base, &entity_path) {
                    if let Some(props) = overrides.get_opt(&prefix) {
                        resolved_properties = resolved_properties.with_child(props);
                    }
                }

                let override_path = base_entity_path.join(&props_path).join(&entity_path);
                let individual_properties = overrides.get_opt(&entity_path).cloned();

                data_results.insert(DataResultNode {
                    data_result: DataResult {
                        entity_path,
                        view_parts,
                        resolved_properties,
                        individual_properties,
                        override_path,
                    },
                    children: Default::default(),
                })
            })
            .collect();

        // And then append the recursive children
        let mut recursive_children: BTreeSet<DataResultHandle> = self
            .children
            .iter()
            .filter_map(|handle| {
                contents.group(*handle).map(|group| {
                    group.add_to_data_results_recursive(
                        contents,
                        overrides,
                        inherited_base,
                        inherited,
                        data_results,
                    )
                })
            })
            .collect();

        children.append(&mut recursive_children);

        let individual_properties = overrides.get_opt(&group_path).cloned();
        data_results.insert(DataResultNode {
            data_result: DataResult {
                entity_path: group_path,
                view_parts: group_view_parts,
                resolved_properties: group_resolved_properties,
                individual_properties,
                override_path: group_override_path,
            },
            children,
        })
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_incremental_walk() {
    assert_eq!(
        incremental_walk(None, &EntityPath::root()).collect::<Vec<_>>(),
        vec![EntityPath::root()]
    );
    assert_eq!(
        incremental_walk(Some(&EntityPath::root()), &EntityPath::root()).collect::<Vec<_>>(),
        vec![]
    );
    assert_eq!(
        incremental_walk(None, &EntityPath::from("foo")).collect::<Vec<_>>(),
        vec![EntityPath::root(), EntityPath::from("foo")]
    );
    assert_eq!(
        incremental_walk(Some(&EntityPath::root()), &EntityPath::from("foo")).collect::<Vec<_>>(),
        vec![EntityPath::from("foo")]
    );
    assert_eq!(
        incremental_walk(None, &EntityPath::from("foo/bar")).collect::<Vec<_>>(),
        vec![
            EntityPath::root(),
            EntityPath::from("foo"),
            EntityPath::from("foo/bar")
        ]
    );
    assert_eq!(
        incremental_walk(
            Some(&EntityPath::from("foo")),
            &EntityPath::from("foo/bar/baz")
        )
        .collect::<Vec<_>>(),
        vec![EntityPath::from("foo/bar"), EntityPath::from("foo/bar/baz")]
    );
}
