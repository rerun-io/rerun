use crate::{ArchetypeDefinition, SpaceViewSystemExecutionError, ViewQuery, ViewerContext};

use super::view_context_system::ViewContextCollection;

/// Scene part collection, consisting of several [`ViewPartSystem`] which may be populated in parallel.
pub trait ViewPartSystemCollection {
    /// Retrieves a list of all underlying scene context part for parallel population.
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewPartSystem>;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    ///
    /// TODO(andreas): don't pass in `ViewerContext` if we want to restrict the queries here.
    /// If we want to make this restriction, then the trait-contract should be that something external
    /// to the `ViewPartSystemImpl` does the query and then passes an `ArchetypeQueryResult` into populate.
    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to several parts of a [`crate::SpaceViewClass`].
    /// For example, if most scene parts produce ui elements, a concrete [`crate::SpaceViewClass`]
    /// can pick those up in its [`crate::SpaceViewClass::ui`] method by iterating over all scene parts.
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any;
}

/// Trivial implementation of a scene collection that consists only of a single scene part.
impl<T: ViewPartSystem + 'static> ViewPartSystemCollection for T {
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewPartSystem> {
        vec![self]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
