use crate::{
    ArchetypeDefinition, SceneContext, SceneQuery, SpaceViewHighlights, SpaceViewState,
    ViewerContext,
};

/// Scene part collection, consisting of several [`ScenePart`] which may be populated in parallel.
pub trait ScenePartCollection {
    /// Retrieves a list of all underlying scene context part for parallel population.
    fn vec_mut(&mut self) -> Vec<&mut dyn ScenePart>;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ScenePart: std::any::Any {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &dyn SpaceViewState,
        context: &dyn SceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is a useful for retrieving a data struct that may be common for all scene elements
    /// of a particular [`crate::SpaceViewClass`].
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}
