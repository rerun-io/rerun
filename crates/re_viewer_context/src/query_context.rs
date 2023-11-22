use once_cell::sync::Lazy;
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{DataQueryId, DataResult, ViewerContext};

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

impl Clone for DataQueryResult {
    fn clone(&self) -> Self {
        re_tracing::profile_function!();
        Self {
            id: self.id,
            tree: self.tree.clone(),
        }
    }
}

impl Default for DataQueryResult {
    fn default() -> Self {
        Self {
            id: DataQueryId::invalid(),
            tree: DataResultTree::default(),
        }
    }
}

/// A hierarchical tree of [`DataResult`]s
#[derive(Clone, Default)]
pub struct DataResultTree {
    pub data_results: SlotMap<DataResultHandle, DataResultNode>,
    pub root_handle: Option<DataResultHandle>,
}

/// A single node in the [`DataResultTree`]
#[derive(Clone, Debug)]
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

static EMPTY_QUERY: Lazy<DataQueryResult> = Lazy::<DataQueryResult>::new(Default::default);

impl ViewerContext<'_> {
    pub fn lookup_query_result(&self, id: DataQueryId) -> &DataQueryResult {
        self.query_results.get(&id).unwrap_or_else(|| {
            re_log::warn!("Missing query!");
            &EMPTY_QUERY
        })
    }
}
