use re_entity_db::{external::re_data_store::LatestAtQuery, EntityProperties, EntityPropertyMap};
use re_viewer_context::{DataQueryResult, PerVisualizer, StoreContext, VisualizableEntities};

pub struct EntityOverrideContext {
    pub root: EntityProperties,
    pub individual: EntityPropertyMap,
    pub group: EntityPropertyMap,
}

/// Trait for resolving properties needed by most implementations of [`DataQuery`]
///
/// The `SpaceViewBlueprint` is the only thing that likely implements this today
/// but we use a trait here so we don't have to pick up a full dependency on `re_viewport`.
pub trait PropertyResolver {
    fn update_overrides(
        &self,
        ctx: &StoreContext<'_>,
        query: &LatestAtQuery,
        query_result: &mut DataQueryResult,
    );
}

/// The common trait implemented for data queries
///
/// Both interfaces return [`re_viewer_context::DataResult`]s, which are self-contained description of the data
/// to be added to a `SpaceView` including both the [`re_log_types::EntityPath`] and context for any overrides.
pub trait DataQuery {
    /// Execute a full query, returning a `DataResultTree` containing all results.
    ///
    /// `auto_properties` is a map containing any heuristic-derived auto properties for the given `SpaceView`.
    ///
    /// This is used when building up the contents for a `SpaceView`.
    fn execute_query(
        &self,
        ctx: &StoreContext<'_>,
        visualizable_entities_for_visualizer_systems: &PerVisualizer<VisualizableEntities>,
    ) -> DataQueryResult;
}
