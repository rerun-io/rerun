//! Various strongly typed sets of entities to express intent and avoid mistakes.

use ahash::HashMap;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use re_chunk::ComponentIdentifier;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_types_core::ViewClassIdentifier;

use crate::ViewSystemIdentifier;

/// Types of matches when matching [`crate::RequiredComponents::AnyPhysicalDatatype`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatatypeMatchKind {
    /// Only the physical datatype was matched, but semantics aren't the native ones.
    PhysicalDatatypeOnly,

    /// The Rerun native datatype was matched.
    ///
    /// For example the native type for a Rerun point cloud is `rerun.components.Position3D`.
    /// This is *not* concerned with the column name of the data, only the datatype.
    NativeSemantics,
}

/// Describes why a given entity was marked as visualizable.
#[derive(Clone, Debug)]
pub enum VisualizableReason {
    /// The entity is visualizable because all entities are visualizable for this type.
    Always,

    /// [`crate::RequiredComponents::AllComponents`] matched for this entity.
    ExactMatchAll,

    /// [`crate::RequiredComponents::AnyComponent`] matched for this entity.
    ExactMatchAny,

    /// [`crate::RequiredComponents::AnyPhysicalDatatype`] matched for this entity with the given components.
    DatatypeMatchAny {
        matches: IntMap<ComponentIdentifier, DatatypeMatchKind>,
    },
}

impl VisualizableReason {
    /// Returns true if this reason includes a match with native semantics.
    ///
    /// The component identifier of the match may not be equal to the one the visualizer's associated archetype expects.
    /// To ensure that, use [`Self::full_native_match`].
    /// This distinction is only ever relevant if we want to distinguish with the component name as well.
    ///
    /// Example:
    /// `SeriesLines` visualizer expects a component named "Scalars:scalars" with component (semantic) type "rerun.components.Scalars".
    /// For an incoming entity with a component named "OtherScalars:scalars" with component type "rerun.components.Scalars",
    /// [`Self::any_match_with_native_semantics`] would return true, but [`Self::full_native_match`] would return false.
    //
    // TODO(andreas): We'll likely move into a direction where semantic match will always be sufficient and emits a mapping to the
    // component if needed (not needed == full_native_match) which should be preferred when possible.
    pub fn any_match_with_native_semantics(&self) -> bool {
        match self {
            Self::Always | Self::ExactMatchAll | Self::ExactMatchAny => true,
            Self::DatatypeMatchAny { matches } => matches
                .values()
                .contains(&DatatypeMatchKind::NativeSemantics),
        }
    }

    /// Returns true if this match reason is a perfect match for the given component identifier.
    pub fn full_native_match(&self, component_identifier: ComponentIdentifier) -> bool {
        match self {
            Self::Always | Self::ExactMatchAll | Self::ExactMatchAny => true,
            Self::DatatypeMatchAny { matches } => {
                matches.get(&component_identifier) == Some(&DatatypeMatchKind::NativeSemantics)
            }
        }
    }
}

/// List of entities that are visualizable with a given visualizer.
///
/// Note that this filter latches:
/// An entity is marked visualizable if it at any point in time on any timeline has all required components.
///
/// We evaluate this filtering step entirely by store subscriber and provide a reason
/// for why this entity was deemed visualizable. This in turn implies that this can
/// *not* be influenced by individual view setups.
#[derive(Default, Clone, Debug)]
pub struct VisualizableEntities(pub IntMap<EntityPath, VisualizableReason>);

impl std::ops::Deref for VisualizableEntities {
    type Target = IntMap<EntityPath, VisualizableReason>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of entities that contain archetypes that are relevant for a visualizer.
///
/// In order to be a match the entity must have at some point in time on any timeline had any
/// component that had an associated archetype as specified by the respective visualizer system.
#[derive(Default, Clone, Debug)]
pub struct IndicatedEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for IndicatedEntities {
    type Target = IntSet<EntityPath>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of elements per visualizer system.
///
/// Careful, if you're in the context of a view, this may contain visualizers that aren't relevant to the current view.
/// Refer to [`PerVisualizerTypeInViewClass`] for a collection that is limited to visualizers active for a given view.
#[derive(Debug)]
pub struct PerVisualizerType<T>(pub IntMap<ViewSystemIdentifier, T>);

impl<T> std::ops::Deref for PerVisualizerType<T> {
    type Target = IntMap<ViewSystemIdentifier, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Clone> Clone for PerVisualizerType<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Manual default impl, otherwise T: Default would be required.
impl<T> Default for PerVisualizerType<T> {
    fn default() -> Self {
        Self(IntMap::default())
    }
}

impl<T> re_byte_size::SizeBytes for PerVisualizerType<T>
where
    T: re_byte_size::SizeBytes,
{
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

/// Like [`PerVisualizerType`], but ensured that all visualizers are relevant for the given view class.
#[derive(Debug)]
pub struct PerVisualizerTypeInViewClass<T> {
    /// View for which this list is filtered down.
    ///
    /// Most of the time we don't actually need this field but it is useful for debugging
    /// and ensuring that [`Self::per_visualizer`] is scoped down to this view.
    pub view_class_identifier: ViewClassIdentifier,

    /// Items per visualizer system.
    pub per_visualizer: IntMap<ViewSystemIdentifier, T>,
}

impl<T> PerVisualizerTypeInViewClass<T> {
    pub fn empty(view_class_identifier: ViewClassIdentifier) -> Self {
        Self {
            view_class_identifier,
            per_visualizer: Default::default(),
        }
    }
}

impl<T> std::ops::Deref for PerVisualizerTypeInViewClass<T> {
    type Target = IntMap<ViewSystemIdentifier, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.per_visualizer
    }
}

/// List of elements per visualizer instruction id.
#[derive(Debug)]
pub struct PerVisualizerInstruction<T>(pub HashMap<VisualizerInstructionId, T>);

impl<T> std::ops::Deref for PerVisualizerInstruction<T> {
    type Target = HashMap<VisualizerInstructionId, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for PerVisualizerInstruction<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Clone> Clone for PerVisualizerInstruction<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Manual default impl, otherwise T: Default would be required.
impl<T> Default for PerVisualizerInstruction<T> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<T> re_byte_size::SizeBytes for PerVisualizerInstruction<T>
where
    T: re_byte_size::SizeBytes,
{
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}
