use std::collections::BTreeSet;

use re_viewer_context::{DataResult, DynSpaceViewClass};
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
    // TODO(jleibs): Needs access to the store
    fn execute_query(&self, class: &dyn DynSpaceViewClass) -> DataResultTree;
}

impl DataBlueprintGroup {
    fn to_data_result(
        &self,
        contents: &SpaceViewContents,
        data_results: &mut SlotMap<DataResultHandle, DataResultNode>,
    ) -> DataResultHandle {
        let entity_path = self.group_path.clone();

        // TODO(jleibs): This remapping isn't great when a view has a bunch of entity-types.
        let view_parts = contents
            .per_system_entities()
            .iter()
            .filter_map(|(part, ents)| {
                if ents.contains(&entity_path) {
                    Some(*part)
                } else {
                    None
                }
            })
            .collect();

        let resolved_properties = contents.data_blueprints_projected().get(&entity_path);

        let mut children: BTreeSet<DataResultHandle> = self
            .entities
            .iter()
            .filter(|entity| entity_path != **entity)
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

                let resolved_properties = contents.data_blueprints_projected().get(entity_path);

                data_results.insert(DataResultNode {
                    data_result: DataResult {
                        entity_path: entity_path.clone(),
                        view_parts,
                        resolved_properties,
                    },
                    children: Default::default(),
                })
            })
            .collect();

        let mut recursive_children: BTreeSet<DataResultHandle> = self
            .children
            .iter()
            .filter_map(|handle| {
                contents
                    .group(*handle)
                    .map(|group| group.to_data_result(contents, data_results))
            })
            .collect();

        children.append(&mut recursive_children);

        data_results.insert(DataResultNode {
            data_result: DataResult {
                entity_path,
                view_parts,
                resolved_properties,
            },
            children,
        })
    }
}

impl DataQuery for SpaceViewContents {
    fn execute_query(&self, _class: &dyn DynSpaceViewClass) -> DataResultTree {
        let mut data_results = SlotMap::<DataResultHandle, DataResultNode>::default();
        let root_handle = self.root_group().to_data_result(self, &mut data_results);
        DataResultTree {
            data_results,
            root_handle,
        }
    }
}
