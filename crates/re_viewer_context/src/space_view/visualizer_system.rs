use ahash::HashMap;

use re_types::{Archetype, ComponentNameSet};

use crate::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewSystemIdentifier, ViewerContext, VisualizableEntities,
    VisualizableFilterContext, VisualizerAdditionalApplicabilityFilter,
};

pub struct VisualizerQueryInfo {
    /// These are not required, but if _any_ of these are found, it is a strong indication that this
    /// system should be active (if also the `required_components` are found).
    pub indicators: ComponentNameSet,

    /// Returns the minimal set of components that the system _requires_ in order to be instantiated.
    ///
    /// This does not include indicator components.
    pub required: ComponentNameSet,

    /// Returns the set of components that the system _queries_.
    /// Must include required, usually excludes indicators
    pub queried: ComponentNameSet,
}

impl VisualizerQueryInfo {
    pub fn from_archetype<T: Archetype>() -> Self {
        Self {
            indicators: std::iter::once(T::indicator().name()).collect(),
            required: T::required_components()
                .iter()
                .map(ToOwned::to_owned)
                .collect(),
            queried: T::all_components().iter().map(ToOwned::to_owned).collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            indicators: ComponentNameSet::new(),
            required: ComponentNameSet::new(),
            queried: ComponentNameSet::new(),
        }
    }
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait VisualizerSystem: Send + Sync + 'static {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// Information about which components are queried by the visualizer.
    fn visualizer_query_info(&self) -> VisualizerQueryInfo;

    /// Filters a set of applicable entities (entities that have all required components),
    /// into to a set of visualizable entities.
    ///
    /// The context passed in here is generated by [`crate::SpaceViewClass::visualizable_filter_context`].
    #[inline]
    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        _context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        VisualizableEntities(entities.0)
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
    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to several visualizers of a [`crate::SpaceViewClass`].
    /// For example, if most visualizers produce ui elements, a concrete [`crate::SpaceViewClass`]
    /// can pick those up in its [`crate::SpaceViewClass::ui`] method by iterating over all visualizers.
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns an initial value to use when creating an override for a component for this
    /// visualizer. This is used as a fallback if the component doesn't already have data.
    fn initial_override_value(
        &self,
        _ctx: &ViewerContext<'_>,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
        _entity_path: &re_log_types::EntityPath,
        _component: &re_types::ComponentName,
    ) -> Option<re_log_types::DataCell> {
        None
    }
}

pub struct VisualizerCollection {
    pub systems: HashMap<ViewSystemIdentifier, Box<dyn VisualizerSystem>>,
}

impl VisualizerCollection {
    #[inline]
    pub fn get<T: VisualizerSystem + IdentifiedViewSystem + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&T::identifier())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                SpaceViewSystemExecutionError::VisualizerSystemNotFound(T::identifier().as_str())
            })
    }

    #[inline]
    pub fn get_by_identifier(
        &self,
        name: ViewSystemIdentifier,
    ) -> Result<&dyn VisualizerSystem, SpaceViewSystemExecutionError> {
        self.systems
            .get(&name)
            .map(|s| s.as_ref())
            .ok_or_else(|| SpaceViewSystemExecutionError::VisualizerSystemNotFound(name.as_str()))
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &dyn VisualizerSystem> {
        self.systems.values().map(|s| s.as_ref())
    }

    #[inline]
    pub fn iter_with_identifiers(
        &self,
    ) -> impl Iterator<Item = (ViewSystemIdentifier, &dyn VisualizerSystem)> {
        self.systems.iter().map(|s| (*s.0, s.1.as_ref()))
    }
}
