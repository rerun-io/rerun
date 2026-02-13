use std::sync::LazyLock;

use ahash::HashMap;
use nohash_hasher::IntMap;
use re_log_types::{EntityPath, EntityPathHash};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::{
    DataResult, StoreContext, ViewContext, ViewId, ViewState, ViewerContext, blueprint_timeline,
};

slotmap::new_key_type! {
    /// Identifier for a [`DataResultNode`]
    pub struct DataResultHandle;
}

/// Context for a latest-at query in a specific view.
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

    /// If the query is made from a visualizer, this contains that visualizer's id.
    pub instruction_id: Option<VisualizerInstructionId>,

    /// Archetype name in which context the component is needed.
    ///
    /// View properties always have an archetype context, but overrides/defaults may not.
    pub archetype_name: Option<re_sdk_types::ArchetypeName>,

    /// Query which didn't yield a result for the component at the target entity path.
    pub query: re_chunk_store::LatestAtQuery,
}

impl QueryContext<'_> {
    #[inline]
    pub fn viewer_ctx(&self) -> &ViewerContext<'_> {
        self.view_ctx.viewer_ctx
    }

    #[inline]
    pub fn store_ctx(&self) -> &StoreContext<'_> {
        self.view_ctx.viewer_ctx.store_context
    }

    #[inline]
    pub fn render_ctx(&self) -> &re_renderer::RenderContext {
        self.view_ctx.viewer_ctx.global_context.render_ctx
    }

    #[inline]
    pub fn egui_ctx(&self) -> &egui::Context {
        self.view_ctx.viewer_ctx.global_context.egui_ctx
    }

    #[inline]
    pub fn recording(&self) -> &re_entity_db::EntityDb {
        self.view_ctx.recording()
    }

    #[inline]
    pub fn view_state(&self) -> &dyn ViewState {
        self.view_ctx.view_state
    }
}

/// The result of executing a single data query for a specific view.
#[derive(Debug)]
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

    /// Latest-at results for all component defaults in this view.
    pub view_defaults: re_query::LatestAtResults,
}

impl Default for DataQueryResult {
    fn default() -> Self {
        Self {
            tree: Default::default(),
            num_matching_entities: 0,
            num_visualized_entities: 0,
            view_defaults: re_query::LatestAtResults::empty(
                "<defaults>".into(),
                re_chunk_store::LatestAtQuery::latest(blueprint_timeline()),
            ),
        }
    }
}

impl DataQueryResult {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    #[inline]
    pub fn result_for_entity(&self, path: &EntityPath) -> Option<&DataResult> {
        self.tree
            .lookup_result_by_path(path.hash())
            .filter(|result| !result.tree_prefix_only)
    }
}

impl Clone for DataQueryResult {
    fn clone(&self) -> Self {
        re_tracing::profile_function!();
        Self {
            tree: self.tree.clone(),
            num_matching_entities: self.num_matching_entities,
            num_visualized_entities: self.num_visualized_entities,
            view_defaults: self.view_defaults.clone(),
        }
    }
}

/// A hierarchical tree of [`DataResult`]s
#[derive(Clone, Default, Debug)]
pub struct DataResultTree {
    pub data_results: SlotMap<DataResultHandle, DataResultNode>,
    pub data_results_by_path: IntMap<EntityPathHash, DataResultHandle>,
    pub data_results_by_visualizer_instruction: HashMap<VisualizerInstructionId, DataResultHandle>,

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
            // Filled in later.
            // TODO(andreas): This is super messy: we rely on this being filled out by `DataQueryPropertyResolver::update_overrides`.
            // At the time `DataResultTree::new` is called we don't have any information about visualizer instructions yet.
            // Really the underlying problem is that we for no apparent reason separate creation of data results from
            // creation of visualizer instructions & determination of available overrides.
            // Merging those two tree walks should make things also a lot more efficient and even parallelizable if we want.
            data_results_by_visualizer_instruction: Default::default(),
        }
    }

    pub fn root_handle(&self) -> Option<DataResultHandle> {
        self.root_handle
    }

    pub fn root_node(&self) -> Option<&DataResultNode> {
        self.data_results.get(self.root_handle?)
    }

    /// Depth-first traversal of the tree, calling `visitor` on each result.
    ///
    /// Stops traversing a branch if `visitor` returns `false`.
    pub fn visit<'a>(&'a self, visitor: &mut impl FnMut(&'a DataResultNode) -> bool) {
        if let Some(root_handle) = self.root_handle {
            self.visit_recursive(root_handle, visitor);
        }
    }

    /// Depth-first traversal of the tree, calling `visitor` on each result, starting from a
    /// specific node.
    ///
    /// Stops traversing a branch if `visitor` returns `false`.
    pub fn visit_from_node<'a>(
        &'a self,
        node: &DataResultNode,
        visitor: &mut impl FnMut(&'a DataResultNode) -> bool,
    ) {
        if let Some(handle) = self
            .data_results_by_path
            .get(&node.data_result.entity_path.hash())
        {
            self.visit_recursive(*handle, visitor);
        }
    }

    /// Depth-first search of a node based on the provided predicate.
    ///
    /// If a `staring_node` is provided, the search starts at that node. Otherwise, it starts at the
    /// root node.
    pub fn find_node_by(
        &self,
        starting_node: Option<&DataResultNode>,
        predicate: impl Fn(&DataResultNode) -> bool,
    ) -> Option<&DataResultNode> {
        let mut result = None;

        let node = starting_node.or_else(|| self.root_node())?;
        self.visit_from_node(node, &mut |node| {
            if predicate(node) {
                result = Some(node);
            }

            // keep recursing until we find something
            result.is_none()
        });
        result
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

    /// Look up a [`DataResultNode`] in the tree based on an [`EntityPathHash`].
    #[inline]
    pub fn lookup_node_by_path(&self, path: EntityPathHash) -> Option<&DataResultNode> {
        self.lookup_node(*self.data_results_by_path.get(&path)?)
    }

    /// Look up a [`DataResult`] in the tree based on an [`EntityPathHash`].
    #[inline]
    pub fn lookup_result_by_path(&self, path: EntityPathHash) -> Option<&DataResult> {
        self.lookup_result(*self.data_results_by_path.get(&path)?)
    }

    /// Look up a [`DataResultNode`] in the tree based on a visualizer instruction ID.
    #[inline]
    pub fn lookup_result_by_visualizer_instruction(
        &self,
        visualizer_instruction: VisualizerInstructionId,
    ) -> Option<&DataResult> {
        self.lookup_result(
            *self
                .data_results_by_visualizer_instruction
                .get(&visualizer_instruction)?,
        )
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
        if let Some(result) = self.data_results.get(handle)
            && visitor(result)
        {
            for child in &result.children {
                self.visit_recursive(*child, visitor);
            }
        }
    }

    /// Iterates over all [`DataResult`]s.
    #[inline]
    pub fn iter_data_results(&self) -> impl Iterator<Item = &DataResult> {
        self.data_results.values().map(|node| &node.data_result)
    }
}

static EMPTY_QUERY: LazyLock<DataQueryResult> = LazyLock::new(Default::default);

impl ViewerContext<'_> {
    pub fn lookup_query_result(&self, id: ViewId) -> &DataQueryResult {
        self.query_results.get(&id).unwrap_or_else(|| {
            re_log::debug_warn!("Tried looking up a query that doesn't exist: {id:?}");
            &EMPTY_QUERY
        })
    }
}
