use std::collections::BTreeMap;

use nohash_hasher::IntSet;

use re_types::{Archetype, ComponentDescriptor};

use crate::{
    ApplicableEntities, ComponentFallbackProvider, IdentifiedViewSystem, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    VisualizableEntities, VisualizableFilterContext, VisualizerAdditionalApplicabilityFilter,
};

// TODO: nuh-huh

#[derive(Debug, Clone, Default)]
pub struct SortedComponentDescriptorSet(linked_hash_map::LinkedHashMap<ComponentDescriptor, ()>);

impl SortedComponentDescriptorSet {
    pub fn insert(&mut self, k: ComponentDescriptor) -> Option<()> {
        self.0.insert(k, ())
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = ComponentDescriptor>) {
        self.0.extend(iter.into_iter().map(|k| (k, ())));
    }

    pub fn iter(&self) -> linked_hash_map::Keys<'_, ComponentDescriptor, ()> {
        self.0.keys()
    }

    pub fn contains(&self, k: &ComponentDescriptor) -> bool {
        self.0.contains_key(k)
    }
}

impl FromIterator<ComponentDescriptor> for SortedComponentDescriptorSet {
    fn from_iter<I: IntoIterator<Item = ComponentDescriptor>>(iter: I) -> Self {
        Self(iter.into_iter().map(|k| (k, ())).collect())
    }
}

// TODO: surely that's the issue... we need to be querying descriptors, not these names.
pub struct VisualizerQueryInfo {
    /// These are not required, but if _any_ of these are found, it is a strong indication that this
    /// system should be active (if also the `required_components` are found).
    pub indicators: IntSet<ComponentDescriptor>,

    /// Returns the minimal set of components that the system _requires_ in order to be instantiated.
    ///
    /// This does not include indicator components.
    pub required: IntSet<ComponentDescriptor>,

    /// Returns the list of components that the system _queries_.
    ///
    /// Must include required, usually excludes indicators.
    /// Order should reflect order in archetype docs & user code as well as possible.
    pub queried: SortedComponentDescriptorSet,
}

impl VisualizerQueryInfo {
    pub fn from_archetype<T: Archetype>() -> Self {
        use re_types_core::ComponentBatch as _;
        Self {
            indicators: std::iter::once(T::indicator().descriptor().into_owned()).collect(),
            required: T::required_components().iter().cloned().collect(),
            queried: T::all_components().iter().cloned().collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            indicators: Default::default(),
            required: Default::default(),
            queried: SortedComponentDescriptorSet::default(),
        }
    }
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
///
/// All visualizers are expected to be able to provide a fallback value for any component they're using
/// via the [`ComponentFallbackProvider`] trait.
pub trait VisualizerSystem: Send + Sync + 'static {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// Information about which components are queried by the visualizer.
    fn visualizer_query_info(&self) -> VisualizerQueryInfo;

    /// Filters a set of applicable entities (entities that have all required components),
    /// into to a set of visualizable entities.
    ///
    /// The context passed in here is generated by [`crate::ViewClass::visualizable_filter_context`].
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

    /// Queries the chunk store and performs data conversions to make it ready for display.
    ///
    /// Mustn't query any data outside of the archetype.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError>;

    /// Optionally retrieves a chunk store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to several visualizers of a [`crate::ViewClass`].
    /// For example, if most visualizers produce ui elements, a concrete [`crate::ViewClass`]
    /// can pick those up in its [`crate::ViewClass::ui`] method by iterating over all visualizers.
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns the fallback provider for this visualizer.
    ///
    /// Visualizers should use this to report the fallback values they use when there is no data.
    /// The Rerun viewer will display these fallback values to the user to convey what the
    /// visualizer is doing.
    fn fallback_provider(&self) -> &dyn ComponentFallbackProvider;
}

pub struct VisualizerCollection {
    pub systems: BTreeMap<ViewSystemIdentifier, Box<dyn VisualizerSystem>>,
}

impl VisualizerCollection {
    #[inline]
    pub fn get<T: VisualizerSystem + IdentifiedViewSystem + 'static>(
        &self,
    ) -> Result<&T, ViewSystemExecutionError> {
        self.systems
            .get(&T::identifier())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                ViewSystemExecutionError::VisualizerSystemNotFound(T::identifier().as_str())
            })
    }

    #[inline]
    pub fn get_by_identifier(
        &self,
        name: ViewSystemIdentifier,
    ) -> Result<&dyn VisualizerSystem, ViewSystemExecutionError> {
        self.systems
            .get(&name)
            .map(|s| s.as_ref())
            .ok_or_else(|| ViewSystemExecutionError::VisualizerSystemNotFound(name.as_str()))
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

    /// Iterate over all visualizer data that can be downcast to the given type.
    pub fn iter_visualizer_data<SpecificData: 'static>(
        &self,
    ) -> impl Iterator<Item = &'_ SpecificData> {
        self.iter().filter_map(|visualizer| {
            visualizer
                .data()
                .and_then(|data| data.downcast_ref::<SpecificData>())
        })
    }
}
