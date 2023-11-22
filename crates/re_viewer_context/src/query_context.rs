use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{DataQueryId, DataResult};

slotmap::new_key_type! {
    /// Identifier for a [`DataResultNode`]
    pub struct DataResultHandle;
}

/// The result of executing a single data query
pub struct DataQueryResult {
    /// Which [`DataQuery`] generated this result
    pub id: DataQueryId,

    /// The [`DataResultTree`] for the query
    pub tree: DataResultTree,
}

/// A hierarchical tree of [`DataResult`]s
pub struct DataResultTree {
    pub data_results: SlotMap<DataResultHandle, DataResultNode>,
    pub root_handle: Option<DataResultHandle>,
}

/// A single node in the [`DataResultTree`]
#[derive(Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    pub children: SmallVec<[DataResultHandle; 4]>,
}

impl DataResultTree {
    /// Depth-first traversal of the tree, calling `visitor` on each result.
    pub fn visit(&self, visitor: &mut impl FnMut(DataResultHandle)) {
        if let Some(root_handle) = self.root_handle {
            self.visit_recursive(root_handle, visitor);
        }
    }

    /// Look up a [`DataResult`] in the tree based on its handle.
    pub fn lookup_result(&self, handle: DataResultHandle) -> Option<&DataResult> {
        self.data_results.get(handle).map(|node| &node.data_result)
    }

    /// Look up a [`DataResultNode`] in the tree based on its handle.
    pub fn lookup_node(&self, handle: DataResultHandle) -> Option<&DataResultNode> {
        self.data_results.get(handle)
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
