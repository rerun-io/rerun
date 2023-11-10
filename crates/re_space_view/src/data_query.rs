use std::collections::BTreeSet;

use re_data_store::{EntityPath, EntityProperties, EntityPropertiesComponent, EntityPropertyMap};
use re_viewer_context::{DataResult, SpaceViewId, ViewerContext};
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

/// The common trait implemented for data queries
///
/// Both interfaces return [`DataResult`]s, which are self-contained description of the data
/// to be added to a `SpaceView` including both the [`EntityPath`] and context for any overrides.
pub trait DataQuery {
    /// Execute a full query, returning a [`DataResultTree`] containing all results.
    ///
    /// `auto_properties` is a map containing any heuristic-derived auto properties for the given `SpaceView`.
    ///
    /// This is used when building up the contents for a `SpaceView`.
    fn execute_query(
        &self,
        auto_properties: EntityPropertyMap,
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
        auto_properties: EntityPropertyMap,
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
        auto_properties: EntityPropertyMap,
        ctx: &ViewerContext<'_>,
    ) -> DataResultTree {
        re_tracing::profile_function!();
        let overrides = lookup_entity_properties_for_id(self.space_view_id, auto_properties, ctx);
        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();
        let root_handle = self.root_group().add_to_data_results_recursive(
            self,
            &overrides,
            None,
            &EntityProperties::default(),
            &mut data_results,
        );
        DataResultTree {
            data_results,
            root_handle,
        }
    }

    fn resolve(
        &self,
        auto_properties: EntityPropertyMap,
        ctx: &ViewerContext<'_>,
        entity_path: &EntityPath,
    ) -> DataResult {
        re_tracing::profile_function!();
        let overrides = lookup_entity_properties_for_id(self.space_view_id, auto_properties, ctx);

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

        let mut resolved_properties = EntityProperties::default();
        for prefix in incremental_walk(None, entity_path) {
            resolved_properties = resolved_properties.with_child(&overrides.get(&prefix));
        }

        DataResult {
            entity_path: entity_path.clone(),
            view_parts,
            resolved_properties,
            override_path: self
                .entity_path()
                .join(&"properties".into())
                .join(entity_path),
        }
    }
}

/// Helper function to lookup the properties for a given entity path.
///
/// We start with the auto properties for the `SpaceView` as the base layer and
/// then incrementally override from there.
///
// TODO(jleibs): This should eventually go somewhere more general like `SpaceView`
// but we can't find the `SpaceView` from here -- only the id.
fn lookup_entity_properties_for_id(
    space_view_id: SpaceViewId,
    auto_properties: EntityPropertyMap,
    ctx: &ViewerContext<'_>,
) -> EntityPropertyMap {
    re_tracing::profile_function!();
    let blueprint = ctx.store_context.blueprint;
    let mut prop_map = auto_properties;
    let props_path = space_view_id.as_entity_path().join(&"properties".into());
    if let Some(tree) = blueprint.entity_db().tree.subtree(&props_path) {
        tree.visit_children_recursively(&mut |path: &EntityPath| {
            if let Some(props) = blueprint
                .store()
                .query_timeless_component::<EntityPropertiesComponent>(path)
            {
                let overridden_path =
                    EntityPath::from(&path.as_slice()[props_path.len()..path.len()]);
                prop_map.set(overridden_path, props.value.props);
            }
        });
    }
    prop_map
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
            group_resolved_properties =
                group_resolved_properties.with_child(&overrides.get(&prefix));
        }

        let group_override_path = contents
            .entity_path()
            .join(&"properties".into())
            .join(&group_path);

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
                    resolved_properties = resolved_properties.with_child(&overrides.get(&prefix));
                }

                let override_path = contents
                    .entity_path()
                    .join(&"properties".into())
                    .join(&entity_path);

                data_results.insert(DataResultNode {
                    data_result: DataResult {
                        entity_path,
                        view_parts,
                        resolved_properties,
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

        data_results.insert(DataResultNode {
            data_result: DataResult {
                entity_path: group_path,
                view_parts: group_view_parts,
                resolved_properties: group_resolved_properties,
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
