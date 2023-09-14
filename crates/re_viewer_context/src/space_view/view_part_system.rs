use ahash::HashMap;
use nohash_hasher::IntSet;

use re_log_types::{ComponentName, EntityPath};

use crate::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
    ViewSystemName, ViewerContext,
};

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// Returns the minimal set of components that the system _requires_ in order to be instantiated.
    fn required_components(&self) -> IntSet<ComponentName>;

    /// Implements a filter to heuristically determine whether or not to instantiate the system.
    ///
    /// If and when the system can be instantiated (i.e. because there is at least one entity that satisfies
    /// the minimal set of required components), this method applies an arbitrary filter to determine whether
    /// or not the system should be instantiated by default.
    ///
    /// The passed-in set of `components` corresponds to all the different component that have ever been logged
    /// on the entity path.
    ///
    /// By default, this always returns true.
    /// Override this method only if a more detailed condition is required to inform heuristics whether or not
    /// the given entity is relevant for this system.
    fn heuristic_filter(
        &self,
        _store: &re_arrow_store::DataStore,
        _ent_path: &EntityPath,
        _components: &IntSet<ComponentName>,
    ) -> bool {
        true
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
