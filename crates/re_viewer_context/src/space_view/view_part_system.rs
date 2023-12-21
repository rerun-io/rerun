use ahash::HashMap;

use re_arrow_store::LatestAtQuery;
use re_log_types::EntityPath;
use re_types::ComponentNameSet;

use crate::{
    IdentifiedViewSystem, SpaceViewClassIdentifier, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewQuery, ViewSystemIdentifier, ViewerContext,
    VisualizerAdditionalApplicabilityFilter,
};

/// This is additional context made available to the `heuristic_filter`.
/// It includes tree-context information such as whether certain components
/// exist in the parent hierarchy which are better computed once than having
/// each entity do their own tree-walk.
#[derive(Clone, Copy, Debug)]
pub struct HeuristicFilterContext {
    pub class: SpaceViewClassIdentifier,
    pub has_ancestor_pinhole: bool,
}

impl Default for HeuristicFilterContext {
    fn default() -> HeuristicFilterContext {
        Self {
            class: SpaceViewClassIdentifier::invalid(),
            has_ancestor_pinhole: false,
        }
    }
}

impl HeuristicFilterContext {
    pub fn with_class(&self, class: SpaceViewClassIdentifier) -> Self {
        Self {
            class,
            has_ancestor_pinhole: self.has_ancestor_pinhole,
        }
    }
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem: Send + Sync + 'static {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// Returns the minimal set of components that the system _requires_ in order to be instantiated.
    ///
    /// This does not include indicator components.
    fn required_components(&self) -> ComponentNameSet;

    /// These are not required, but if _any_ of these are found, it is a strong indication that this
    /// system should be active (if also the `required_components` are found).
    #[inline]
    fn indicator_components(&self) -> ComponentNameSet {
        Default::default()
    }

    /// Implements a filter to heuristically determine whether or not to instantiate the system.
    ///
    /// If and when the system can be instantiated (i.e. because there is at least one entity that satisfies
    /// the minimal set of required components), this method applies an arbitrary filter to determine whether
    /// or not the system should be instantiated by default.
    ///
    /// The passed-in set of `entity_components` corresponds to all the different components that have ever
    /// been logged on the entity path.
    ///
    /// By default, this returns true if eiher [`Self::indicator_components`] is empty or
    /// `entity_components` contains at least one of these indicator components.
    ///
    /// Override this method only if a more detailed condition is required to inform heuristics whether or not
    /// the given entity is relevant for this system.
    #[inline]
    fn heuristic_filter(
        &self,
        _store: &re_arrow_store::DataStore,
        _ent_path: &EntityPath,
        _ctx: HeuristicFilterContext,
        _query: &LatestAtQuery,
        entity_components: &ComponentNameSet,
    ) -> bool {
        default_heuristic_filter(entity_components, &self.indicator_components())
    }

    /// Additional filter for applicability.
    ///
    /// If none is specified, applicability is solely determined by required components.
    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        None
    }

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Mustn't query any data outside of the archetype.
    ///
    /// TODO(andreas): don't pass in `ViewerContext` if we want to restrict the queries here.
    /// If we want to make this restriction, then the trait-contract should be that something external
    /// to the `ViewPartSystemImpl` does the query and then passes an `ArchetypeQueryResult` into populate.
    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
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

/// The default implementation for [`ViewPartSystem::heuristic_filter`].
///
/// Returns true if either `indicator_components` is empty or `entity_components` contains at least one
/// of these indicator components.
///
/// Exported as a standalone function to simplify the implementation of custom filters.
#[inline]
pub fn default_heuristic_filter(
    entity_components: &ComponentNameSet,
    indicator_components: &ComponentNameSet,
) -> bool {
    if indicator_components.is_empty() {
        true // if there are no indicator components, then show anything with the required compoonents
    } else {
        // do we have at least one of the indicator components?
        entity_components.intersection(indicator_components).count() > 0
    }
}

pub struct ViewPartCollection {
    pub systems: HashMap<ViewSystemIdentifier, Box<dyn ViewPartSystem>>,
}

impl ViewPartCollection {
    #[inline]
    pub fn get<T: ViewPartSystem + IdentifiedViewSystem + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&T::identifier())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                SpaceViewSystemExecutionError::PartSystemNotFound(T::identifier().as_str())
            })
    }

    #[inline]
    pub fn get_by_identifier(
        &self,
        name: ViewSystemIdentifier,
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
    pub fn iter_with_identifiers(
        &self,
    ) -> impl Iterator<Item = (ViewSystemIdentifier, &dyn ViewPartSystem)> {
        self.systems.iter().map(|s| (*s.0, s.1.as_ref()))
    }
}
