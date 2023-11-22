use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap};
use re_viewer_context::{DataResult, EntitiesPerSystemPerClass, StoreContext};
use slotmap::SlotMap;
use smallvec::SmallVec;

slotmap::new_key_type! {
    /// Identifier for a [`DataResultNode`]
    pub struct DataResultHandle;
}

/// A hierarchical tree of [`DataResult`]s
pub struct DataResultTree {
    pub data_results: SlotMap<DataResultHandle, DataResultNode>,
    pub root_handle: Option<DataResultHandle>,
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

/// A single node in the [`DataResultTree`]
#[derive(Debug)]
pub struct DataResultNode {
    pub data_result: DataResult,
    pub children: SmallVec<[DataResultHandle; 4]>,
}

pub struct EntityOverrides {
    pub root: EntityProperties,
    pub individual: EntityPropertyMap,
    pub group: EntityPropertyMap,
}

/// Trait for resolving properties needed by most implementations of [`DataQuery`]
///
/// The `SpaceViewBlueprint` is the only thing that likely implements this today
/// but we use a trait here so we don't have to pick up a full dependency on `re_viewport`.
pub trait PropertyResolver {
    fn resolve_entity_overrides(&self, ctx: &StoreContext<'_>) -> EntityOverrides;
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
        ctx: &StoreContext<'_>,
        entities_per_system_per_class: &EntitiesPerSystemPerClass,
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
        ctx: &StoreContext<'_>,
        entities_per_system_per_class: &EntitiesPerSystemPerClass,
        entity_path: &EntityPath,
        as_group: bool,
    ) -> DataResult;
}
