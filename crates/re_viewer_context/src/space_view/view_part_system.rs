use ahash::HashMap;
use re_log_types::{ComponentName, EntityPath};

use crate::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewSystemName, ViewerContext,
};

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Returns true if the system queries given components on the given path in its [`Self::execute`] method.
    ///
    /// List of components is expected to be all components that have ever been logged on the entity path.
    /// By default, this only checks if the primary components of the archetype are contained
    /// in the list of components.
    ///
    /// Override this method only if a more detailed condition is required to inform heuristics whether
    /// the given entity is relevant for this system.
    //
    // TODO(andreas): Use new archetype definitions which also allows for several primaries.
    fn queries_any_components_of(
        &self,
        _store: &re_arrow_store::DataStore,
        _ent_path: &EntityPath,
        components: &[ComponentName],
    ) -> bool {
        let archetype = self.archetype();
        components.contains(archetype.first())
    }

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

pub struct ViewPartCollection {
    pub(crate) systems: HashMap<ViewSystemName, Box<dyn ViewPartSystem>>,
}

impl ViewPartCollection {
    #[inline]
    pub fn get<T: ViewPartSystem + NamedViewSystem + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&T::name())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| SpaceViewSystemExecutionError::PartSystemNotFound(T::name().as_str()))
    }

    #[inline]
    pub fn get_by_name(
        &self,
        name: ViewSystemName,
    ) -> Result<&dyn ViewPartSystem, SpaceViewSystemExecutionError> {
        self.systems
            .get(&name)
            .map(|s| s.as_ref())
            .ok_or_else(|| SpaceViewSystemExecutionError::PartSystemNotFound(name.as_str()))
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &dyn ViewPartSystem> {
        self.systems.values().map(|s| s.as_ref())
    }

    #[inline]
    pub fn iter_with_names(&self) -> impl Iterator<Item = (ViewSystemName, &dyn ViewPartSystem)> {
        self.systems.iter().map(|s| (*s.0, s.1.as_ref()))
    }
}
