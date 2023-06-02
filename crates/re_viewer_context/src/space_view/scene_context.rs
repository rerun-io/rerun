use crate::{ArchetypeDefinition, SceneQuery, SpaceViewState, ViewerContext};

/// Scene context, consisting of several [`SceneContextPart`] which may be populated in parallel.
pub trait SceneContext {
    /// Retrieves a list of all underlying scene context part for parallel population.
    fn vec_mut(&mut self) -> Vec<&mut dyn SceneContextPart>;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    /// TODO(wumpf): Only needed for workarounds in `re_space_view_spatial`.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Scene context that can be used by scene elements and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before scene elements.
pub trait SceneContextPart {
    /// Each scene context may query several archetypes.
    ///
    /// This lists all components out that the context queries.
    /// A context may also query no archetypes at all and prepare caches or viewer related data instead.
    fn archetypes(&self) -> Vec<ArchetypeDefinition>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
    );

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
