use crate::blueprint::components as bp_components;
use crate::{AsComponents, SerializedComponentBatch};

/// Configuration for a visualizer in Rerun.
#[derive(Clone, Debug)]
pub struct Visualizer {
    /// Identifies this visualizer instance uniquely within its view.
    pub id: bp_components::VisualizerInstructionId,

    /// The type of visualizer this is.
    pub visualizer_type: bp_components::VisualizerType,

    /// Component overrides that apply to this visualizer.
    pub overrides: Vec<SerializedComponentBatch>,

    /// Component mappings that define how data components map to visualizer parameters.
    ///
    /// TODO(RR-3255): Mappings aren't yet supported.
    pub mappings: Vec<bp_components::VisualizerComponentMapping>,
}

impl Visualizer {
    /// Create a new visualizer configuration with a random id.
    pub fn new(visualizer_type: impl Into<bp_components::VisualizerType>) -> Self {
        Self {
            id: bp_components::VisualizerInstructionId::new_random(),
            visualizer_type: visualizer_type.into(),
            overrides: Vec::new(),
            mappings: Vec::new(),
        }
    }

    /// Add override component batches for this visualizer.
    pub fn with_overrides(mut self, overrides: &impl AsComponents) -> Self {
        self.overrides = overrides.as_serialized_batches();
        self
    }

    /// Add component mappings for this visualizer.
    pub fn with_mappings(
        mut self,
        mappings: impl IntoIterator<Item = bp_components::VisualizerComponentMapping>,
    ) -> Self {
        self.mappings = mappings.into_iter().collect();
        self
    }
}

/// An archetype that has an associated visualizer.
///
/// This applies to most archetypes.
/// Those who don't implement this, like for instance [`crate::archetypes::Transform3D`] may be visualized indirectly via other archetypes.
pub trait VisualizableArchetype {
    /// Create a visualizer for this archetype, using all currently set values as overrides.
    fn visualizer(&self) -> Visualizer;
}

impl<T: VisualizableArchetype + ?Sized> From<&T> for Visualizer {
    fn from(val: &T) -> Self {
        val.visualizer()
    }
}
