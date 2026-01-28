use std::collections::BTreeMap;

use ahash::HashMap;
use re_chunk::{ArchetypeName, ComponentType};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::{Archetype, ComponentDescriptor, ComponentIdentifier, ComponentSet};

use crate::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    ViewSystemIdentifier,
};

#[derive(Debug, Clone, Default)]
pub struct SortedComponentSet(linked_hash_map::LinkedHashMap<ComponentDescriptor, ()>);

impl SortedComponentSet {
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

impl FromIterator<ComponentDescriptor> for SortedComponentSet {
    fn from_iter<I: IntoIterator<Item = ComponentDescriptor>>(iter: I) -> Self {
        Self(iter.into_iter().map(|k| (k, ())).collect())
    }
}

pub type DatatypeSet = std::collections::BTreeSet<arrow::datatypes::DataType>;

/// Specifies how component requirements should be evaluated for visualizer entity matching.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequiredComponents {
    /// No component requirements - all entities are candidates.
    #[default]
    None,

    /// Entity must have _all_ of these components.
    AllComponents(ComponentSet),

    /// Entity must have _any one_ of these components.
    AnyComponent(ComponentSet),

    /// Entity must have _any one_ of these physical Arrow data types.
    ///
    /// For instance, we may not put views into the "recommended" section or visualizer entities proactively unless they support the native type.
    AnyPhysicalDatatype {
        semantic_type: ComponentType,
        physical_types: DatatypeSet,
    },
}

// TODO(grtlr): Eventually we will want to hide these fields to prevent visualizers doing too much shenanigans.
pub struct VisualizerQueryInfo {
    /// This is not required, but if it is found, it is a strong indication that this
    /// system should be active (if also the `required_components` are found).
    pub relevant_archetype: Option<ArchetypeName>,

    /// Returns the minimal set of components that the system _requires_ in order to be instantiated.
    pub required: RequiredComponents,

    /// Returns the list of components that the system _queries_.
    ///
    /// Must include required components.
    /// Order should reflect order in archetype docs & user code as well as possible.
    ///
    /// Note that we need full descriptors here in order to write overrides from the UI.
    pub queried: SortedComponentSet, // TODO(grtlr, wumpf): This can probably be removed?
}

impl VisualizerQueryInfo {
    pub fn from_archetype<A: Archetype>() -> Self {
        Self {
            relevant_archetype: A::name().into(),
            required: RequiredComponents::AllComponents(
                A::required_components()
                    .iter()
                    .map(|c| c.component)
                    .collect(),
            ),
            queried: A::all_components().iter().cloned().collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            relevant_archetype: Default::default(),
            required: RequiredComponents::None,
            queried: SortedComponentSet::default(),
        }
    }

    /// Returns the component _identifiers_ for all queried components.
    pub fn queried_components(&self) -> impl Iterator<Item = ComponentIdentifier> {
        self.queried.iter().map(|desc| desc.component)
    }
}

/// Result of running [`VisualizerSystem::execute`].
#[derive(Default)]
pub struct VisualizerExecutionOutput {
    /// Draw data produced by the visualizer.
    ///
    /// It's the view's responsibility to queue this data for rendering.
    pub draw_data: Vec<re_renderer::QueueableDrawData>,

    /// Errors encountered during execution, mapped to the visualizer instructions that caused them.
    ///
    /// Errors from last frame will be shown in the UI for the respective visualizer instruction.
    /// For errors that prevent any visualization at all, return a
    /// [`ViewSystemExecutionError`] instead.
    pub errors_per_instruction: HashMap<VisualizerInstructionId, String>,
    //
    // TODO(andreas): We should put other output here as well instead of passing around visualizer
    // structs themselves which is rather surprising.
    // Same applies to context systems.
    // This mechanism could easily replace `VisualizerSystem::data`!
}

impl VisualizerExecutionOutput {
    /// Marks the given visualizer instruction as having encountered an error during visualization.
    pub fn report_error_for(
        &mut self,
        instruction_id: VisualizerInstructionId,
        error: impl Into<String>,
    ) {
        self.errors_per_instruction
            .insert(instruction_id, error.into());
    }

    pub fn with_draw_data(
        mut self,
        draw_data: impl IntoIterator<Item = re_renderer::QueueableDrawData>,
    ) -> Self {
        self.draw_data.extend(draw_data);
        self
    }
}

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait VisualizerSystem: Send + Sync + std::any::Any {
    // TODO(andreas): This should be able to list out the ContextSystems it needs.

    /// Information about which components are queried by the visualizer.
    ///
    /// Warning: this method is called on registration of the visualizer system in order
    /// to stear store subscribers. If subsequent calls to this method return different results,
    /// they may not be taken into account.
    fn visualizer_query_info(&self, app_options: &crate::AppOptions) -> VisualizerQueryInfo;

    /// Queries the chunk store and performs data conversions to make it ready for display.
    ///
    /// Mustn't query any data outside of the archetype.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError>;

    /// Optionally retrieves a chunk store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to several visualizers of a [`crate::ViewClass`].
    /// For example, if most visualizers produce ui elements, a concrete [`crate::ViewClass`]
    /// can pick those up in its [`crate::ViewClass::ui`] method by iterating over all visualizers.
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }
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
            .and_then(|s| (s.as_ref() as &dyn std::any::Any).downcast_ref())
            .ok_or_else(|| {
                ViewSystemExecutionError::VisualizerSystemNotFound(T::identifier().as_str())
            })
    }

    #[inline]
    pub fn get_by_type_identifier(
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
        self.iter()
            .filter_map(|visualizer| visualizer.data()?.downcast_ref::<SpecificData>())
    }
}
