use std::collections::BTreeSet;

use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap};
use re_viewer_context::{DataResult, ViewerContext};
use slotmap::SlotMap;

use crate::{DataBlueprintGroup, SpaceViewContents};

slotmap::new_key_type! {
    /// Identifier for a data result.
    pub struct DataResultHandle;
}

pub struct DataResultTree {
    data_results: SlotMap<DataResultHandle, DataResultNode>,
    root_handle: DataResultHandle,
}

impl DataResultTree {
    pub fn visit<F>(&self, mut visitor: F)
    where
        F: FnMut(DataResultHandle),
    {
        let mut stack = Vec::from([self.root_handle]);

        while !stack.is_empty() {
            if let Some(handle) = stack.pop() {
                if let Some(result) = self.data_results.get(handle) {
                    visitor(handle);

                    for child in result.children.iter().rev() {
                        stack.push(*child);
                    }
                }
            }
        }
    }

    pub fn lookup(&self, handle: DataResultHandle) -> Option<&DataResult> {
        self.data_results.get(handle).map(|node| &node.data_result)
    }
}

#[derive(Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    children: BTreeSet<DataResultHandle>,
}

pub trait DataQuery {
    /// Execute query over the entire store to produce all data results.
    fn execute_query(&self, ctx: &ViewerContext<'_>) -> DataResultTree;

    /// Find a single [`DataResult`] within the context of the query.
    fn resolve(&self, ctx: &ViewerContext<'_>, entity_path: &EntityPath) -> DataResult;
}

/// Iterate over all entities from start to end, NOT including start itself.
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

impl DataBlueprintGroup {
    fn to_data_result(
        &self,
        contents: &SpaceViewContents,
        overrides: &EntityPropertyMap,
        inherited_base: Option<&EntityPath>,
        inherited: &EntityProperties,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
    ) -> DataResultHandle {
        let group_path = self.group_path.clone();

        // TODO(jleibs): This remapping isn't great when a view has a bunch of entity-types.
        let view_parts = contents
            .per_system_entities()
            .iter()
            .filter_map(|(part, ents)| {
                if ents.contains(&group_path) {
                    Some(*part)
                } else {
                    None
                }
            })
            .collect();

        let mut resolved_properties = inherited.clone();

        for prefix in incremental_walk(inherited_base, &group_path) {
            resolved_properties = resolved_properties.with_child(&overrides.get(&prefix));
        }

        let mut children: BTreeSet<DataResultHandle> = self
            .entities
            .iter()
            .filter(|entity| group_path != **entity)
            .map(|entity_path| {
                let view_parts = contents
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

                let mut resolved_properties = resolved_properties.clone();

                for prefix in incremental_walk(inherited_base, entity_path) {
                    resolved_properties = resolved_properties.with_child(&overrides.get(&prefix));
                }

                data_results.insert(DataResultNode {
                    data_result: DataResult {
                        entity_path: entity_path.clone(),
                        view_parts,
                        resolved_properties,
                        override_path: contents
                            .entity_path()
                            .join(&"properties".into())
                            .join(entity_path),
                    },
                    children: Default::default(),
                })
            })
            .collect();

        let mut recursive_children: BTreeSet<DataResultHandle> = self
            .children
            .iter()
            .filter_map(|handle| {
                contents.group(*handle).map(|group| {
                    group.to_data_result(
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

        let override_path = contents
            .entity_path()
            .join(&"properties".into())
            .join(&group_path);
        data_results.insert(DataResultNode {
            data_result: DataResult {
                entity_path: group_path,
                view_parts,
                resolved_properties,
                override_path,
            },
            children,
        })
    }
}

impl DataQuery for SpaceViewContents {
    fn execute_query(&self, ctx: &ViewerContext<'_>) -> DataResultTree {
        let overrides = self.lookup_entity_properties(ctx);
        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();
        let root_handle = self.root_group().to_data_result(
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

    fn resolve(&self, ctx: &ViewerContext<'_>, entity_path: &EntityPath) -> DataResult {
        let overrides = self.lookup_entity_properties(ctx);

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
