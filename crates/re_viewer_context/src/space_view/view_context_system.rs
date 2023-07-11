use crate::{ArchetypeDefinition, SceneQuery, SpaceViewState, ViewerContext};

/// Scene context, consisting of several [`ViewContextSystem`] which may be populated in parallel.
pub trait ViewContext {
    /// Retrieves a list of all underlying scene context part for parallel population.
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewContextSystem>;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a mutable reference of [`std::any::Any`], which enables downcasting to concrete types.
    /// TODO(wumpf): Only needed for workarounds in `re_space_view_spatial`.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Implementation of an empty scene context.
impl ViewContext for () {
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewContextSystem> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem {
    /// Each scene context may query several archetypes.
    ///
    /// This lists all archetypes that the context queries.
    /// A context may also query no archetypes at all and prepare caches or viewer related data instead.
    fn archetypes(&self) -> Vec<ArchetypeDefinition>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
    );
}
