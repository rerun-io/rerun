use std::collections::{BTreeMap, BTreeSet};

use re_data_store::{EntityPath, EntityProperties};
use re_viewer_context::{DynSpaceViewClass, SpaceViewClassName, ViewSystemName};
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{DataBlueprintGroup, SpaceViewContents};

slotmap::new_key_type! {
    /// Identifier for a data result.
    pub struct DataResultHandle;
}

#[derive(Debug)]
pub struct DataResult {
    // TODO(jleibs): This should eventually become a more generalized (StoreView + EntityPath) reference to handle
    // multi-RRD or blueprint-static data references.
    pub entity_path: EntityPath,

    pub view_parts: SmallVec<[ViewSystemName; 4]>,

    // TODO(jleibs): Eventually this goes away and becomes implicit as an override layer in the StoreView
    // The reason we store it here though is that context is part of the DataResult.
    pub resolved_properties: EntityProperties,
}

pub struct DataResultTree {
    data_results: SlotMap<DataResultHandle, DataResultNode>,
    root_handle: DataResultHandle,
}

impl DataResultTree {
    pub fn visit<F>(&self, mut visitor: F)
    where
        F: FnMut(&DataResultNode),
    {
        let mut stack = Vec::from([self.root_handle]);

        while !stack.is_empty() {
            if let Some(handle) = stack.pop() {
                if let Some(result) = self.data_results.get(handle) {
                    visitor(result);

                    for child in result.children.iter().rev() {
                        stack.push(*child);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    children: BTreeSet<DataResultHandle>,
}

impl DataResultNode {
    fn new(entity_path: EntityPath) -> DataResultNode {
        DataResultNode {
            data_result: DataResult {
                entity_path,
                view_parts: Default::default(),
                resolved_properties: Default::default(),
            },
            children: Default::default(),
        }
    }
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
            .map(|entity| data_results.insert(DataResultNode::new(entity.clone())))
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
