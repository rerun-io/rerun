use ahash::HashMap;
use once_cell::sync::Lazy;
use re_log_types::EntityPath;
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
    data_results: SlotMap<DataResultHandle, DataResultNode>,
    // TODO(jleibs): Decide if we really want to compute this per-query.
    // at the moment we only look up a single path per frame for the selection panel. It's probably
    // less over-head to just walk the tree once instead of pre-computing an entire map we use for
    // a single lookup.
    data_results_by_path: HashMap<(EntityPath, bool), DataResultHandle>,
    root_handle: Option<DataResultHandle>,
}

/// A single node in the [`DataResultTree`]
#[derive(Clone, Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    pub children: SmallVec<[DataResultHandle; 4]>,
}

impl DataResultTree {
    pub fn new(
        data_results: SlotMap<DataResultHandle, DataResultNode>,
        root_handle: Option<DataResultHandle>,
    ) -> Self {
        re_tracing::profile_function!();
        let data_results_by_path = data_results
            .iter()
            .map(|(handle, node)| {
                (
                    (
                        node.data_result.entity_path.clone(),
                        node.data_result.is_group,
                    ),
                    handle,
                )
            })
            .collect();

        Self {
            data_results,
            data_results_by_path,
            root_handle,
        }
    }

    pub fn root_handle(&self) -> Option<DataResultHandle> {
        self.root_handle
    }

    pub fn root_node(&self) -> Option<&DataResultNode> {
        self.root_handle
            .and_then(|handle| self.data_results.get(handle))
    }

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

    /// Look up a [`DataResultNode`] in the tree based on an [`EntityPath`].
    pub fn lookup_result_by_path_and_group(
        &self,
        path: &EntityPath,
        is_group: bool,
    ) -> Option<&DataResult> {
        self.data_results_by_path
            .get(&(path.clone(), is_group))
            .and_then(|handle| self.lookup_result(*handle))
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
            re_log::debug!("Tried looking up a query that doesn't exist: {:?}", id);
            &EMPTY_QUERY
        })
    }
}
