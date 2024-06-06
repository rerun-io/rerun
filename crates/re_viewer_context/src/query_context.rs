use ahash::HashMap;
use once_cell::sync::Lazy;
use slotmap::SlotMap;
use smallvec::SmallVec;

use re_log_types::{EntityPath, EntityPathHash};

use crate::{DataResult, SpaceViewId, ViewContext, ViewerContext};

slotmap::new_key_type! {
    /// Identifier for a [`DataResultNode`]
    pub struct DataResultHandle;
}

/// Context for a latest at query in a specific view.
// TODO(andreas) this is centered around latest-at queries. Does it have to be? Makes sense for UI, but that means it won't scale much into Visualizer queriers.
// This is currently used only for fallback providers, but the expectation is that we're using this more widely as the primary context object
// in all places where we query a specific entity in a specific view.
pub struct QueryContext<'a> {
    pub view_ctx: &'a ViewContext<'a>,

    /// Target entity path which is lacking the component and needs a fallback.
    ///
    /// For editing overrides/defaults, this is the path to the store entity where they override/default is used.
    /// For view properties this is the path that stores the respective view property archetype.
    pub target_entity_path: &'a re_log_types::EntityPath,

    /// Archetype name in which context the component is needed.
    ///
    /// View properties always have an archetype context, but overrides/defaults may not.
    pub archetype_name: Option<re_types::ArchetypeName>,

    /// Query which didn't yield a result for the component at the target entity path.
    pub query: &'a re_data_store::LatestAtQuery,
}

/// The result of executing a single data query
#[derive(Default)]
pub struct DataQueryResult {
    /// The [`DataResultTree`] for the query
    pub tree: DataResultTree,

    /// The number of entities that matched the query, including those that are not visualizable.
    pub num_matching_entities: usize,

    /// Of the matched queries, the number of entities that are visualizable by any given visualizer.
    ///
    /// This does *not* take into account the actual selection of visualizers
    /// which may be an explicit none for any given entity.
    pub num_visualized_entities: usize,
}

impl DataQueryResult {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    #[inline]
    pub fn contains_entity(&self, path: &EntityPath) -> bool {
        self.tree
            .lookup_result_by_path(path)
            .map_or(false, |result| !result.tree_prefix_only)
    }
}

impl Clone for DataQueryResult {
    fn clone(&self) -> Self {
        re_tracing::profile_function!();
        Self {
            tree: self.tree.clone(),
            num_matching_entities: self.num_matching_entities,
            num_visualized_entities: self.num_visualized_entities,
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
    data_results_by_path: HashMap<EntityPathHash, DataResultHandle>,
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
            .map(|(handle, node)| (node.data_result.entity_path.hash(), handle))
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
    ///
    /// Stops traversing a branch if `visitor` returns `false`.
    pub fn visit<'a>(&'a self, visitor: &mut impl FnMut(&'a DataResultNode) -> bool) {
        if let Some(root_handle) = self.root_handle {
            self.visit_recursive(root_handle, visitor);
        }
    }

    /// Look up a [`DataResult`] in the tree based on its handle.
    #[inline]
    pub fn lookup_result(&self, handle: DataResultHandle) -> Option<&DataResult> {
        self.data_results.get(handle).map(|node| &node.data_result)
    }

    /// Look up a [`DataResultNode`] in the tree based on its handle.
    #[inline]
    pub fn lookup_node(&self, handle: DataResultHandle) -> Option<&DataResultNode> {
        self.data_results.get(handle)
    }

    /// Look up a [`DataResultNode`] in the tree based on its handle.
    #[inline]
    pub fn lookup_node_mut(&mut self, handle: DataResultHandle) -> Option<&mut DataResultNode> {
        self.data_results.get_mut(handle)
    }

    /// Look up a [`DataResultNode`] in the tree based on an [`EntityPath`].
    #[inline]
    pub fn lookup_node_by_path(&self, path: &EntityPath) -> Option<&DataResultNode> {
        self.data_results_by_path
            .get(&path.hash())
            .and_then(|handle| self.lookup_node(*handle))
    }

    /// Look up a [`DataResult`] in the tree based on an [`EntityPath`].
    #[inline]
    pub fn lookup_result_by_path(&self, path: &EntityPath) -> Option<&DataResult> {
        self.data_results_by_path
            .get(&path.hash())
            .and_then(|handle| self.lookup_result(*handle))
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data_results_by_path.is_empty()
    }

    fn visit_recursive<'a>(
        &'a self,
        handle: DataResultHandle,
        visitor: &mut impl FnMut(&'a DataResultNode) -> bool,
    ) {
        if let Some(result) = self.data_results.get(handle) {
            if visitor(result) {
                for child in &result.children {
                    self.visit_recursive(*child, visitor);
                }
            }
        }
    }
}

static EMPTY_QUERY: Lazy<DataQueryResult> = Lazy::<DataQueryResult>::new(Default::default);

impl ViewerContext<'_> {
    pub fn lookup_query_result(&self, id: SpaceViewId) -> &DataQueryResult {
        self.query_results.get(&id).unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                re_log::warn!("Tried looking up a query that doesn't exist: {:?}", id);
            } else {
                re_log::debug!("Tried looking up a query that doesn't exist: {:?}", id);
            }
            &EMPTY_QUERY
        })
    }
}
