use crate::{
    ArchetypeDefinition, SpaceViewClass, SpaceViewHighlights, SpaceViewQuery, ViewerContext,
};

/// Scene part collection, consisting of several [`ViewPartSystem`] which may be populated in parallel.
pub trait ViewPartSystemCollection<C: SpaceViewClass> {
    /// Retrieves a list of all underlying scene context part for parallel population.
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewPartSystem<C>>;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem<C: SpaceViewClass> {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    ///
    /// TODO(andreas): don't pass in `ViewerContext` if we want to restrict the queries here.
    /// If we want to make this restriction, then the trait-contract should be that something external
    /// to the `ViewPartSystemImpl` does the query and then passes an `ArchetypeQueryResult` into populate.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SpaceViewQuery<'_>,
        space_view_state: &C::State,
        context: &C::Context,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to all scene parts of a [`crate::SpaceViewClass`].
    /// For example, if most scene parts produce ui elements, a concrete [`crate::SpaceViewClass`]
    /// can pick those up in its [`crate::SpaceViewClass::ui`] method by iterating over all scene parts.
    fn data(&self) -> Option<&C::ViewPartSystemData> {
        None
    }
}

/// Trivial implementation of a scene collection that consists only of a single scene part.
impl<C: SpaceViewClass, T: ViewPartSystem<C> + 'static> ViewPartSystemCollection<C> for T {
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewPartSystem<C>> {
        vec![self]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
