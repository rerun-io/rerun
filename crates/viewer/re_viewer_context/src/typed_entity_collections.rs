//! Various strongly typed sets of entities to express intent and avoid mistakes.

use ahash::HashMap;
use nohash_hasher::{IntMap, IntSet};
use re_chunk::ComponentIdentifier;
use re_lenses_core::Selector;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_types_core::ViewClassIdentifier;

use crate::ViewSystemIdentifier;

/// Types of matches when matching [`crate::VisualizabilityConstraints::SingleRequiredComponent`].
#[derive(Clone, Debug)]
pub enum DatatypeMatch {
    /// Only the physical datatype was matched, but semantics aren't the native ones.
    PhysicalDatatypeOnly {
        arrow_datatype: arrow::datatypes::DataType,

        /// The semantic component type if any.
        ///
        /// Note that `Some` doesn't necessarily mean that this is a Rerun type, it may still be a user supplied type.
        component_type: Option<re_chunk::ComponentType>,

        /// If this match was found via nested field access, contains the selectors to extract those fields
        /// along with their respective Arrow datatypes for ranking purposes.
        /// Empty for direct matches.
        selectors: Vec<(Selector, arrow::datatypes::DataType)>,
    },

    /// The Rerun native datatype was matched.
    ///
    /// For example the native type for a Rerun point cloud is `rerun.components.Position3D`.
    /// This is *not* concerned with the column name of the data, only the datatype.
    NativeSemantics {
        arrow_datatype: arrow::datatypes::DataType,

        /// The semantic component type if any.
        ///
        /// Note that `Some` doesn't necessarily mean that this is a Rerun type, it may still be a user supplied type.
        component_type: Option<re_chunk::ComponentType>,
    },
}

impl re_byte_size::SizeBytes for DatatypeMatch {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::PhysicalDatatypeOnly {
                arrow_datatype,
                component_type,
                selectors,
            } => {
                arrow_datatype.heap_size_bytes()
                    + component_type.heap_size_bytes()
                    + selectors.heap_size_bytes()
            }
            Self::NativeSemantics {
                arrow_datatype,
                component_type,
            } => arrow_datatype.heap_size_bytes() + component_type.heap_size_bytes(),
        }
    }
}

impl DatatypeMatch {
    pub fn component_type(&self) -> &Option<re_chunk::ComponentType> {
        match self {
            Self::PhysicalDatatypeOnly { component_type, .. }
            | Self::NativeSemantics { component_type, .. } => component_type,
        }
    }

    pub fn arrow_datatype(&self) -> &arrow::datatypes::DataType {
        match self {
            Self::PhysicalDatatypeOnly { arrow_datatype, .. }
            | Self::NativeSemantics { arrow_datatype, .. } => arrow_datatype,
        }
    }
}

/// [`crate::VisualizabilityConstraints::SingleRequiredComponent`] matched for this entity with the given components.
#[derive(Clone, Debug)]
pub struct SingleRequiredComponentMatch {
    /// The component that needs to be mapped to one of the matches.
    pub target_component: ComponentIdentifier,

    /// Matches that the target component should map to.
    ///
    /// Guaranteed to have at least one entry.
    pub matches: IntMap<ComponentIdentifier, DatatypeMatch>,
}

/// [`crate::VisualizabilityConstraints::BufferAndFormat`] matched for this entity.
///
/// Both a buffer component (matched by arrow datatype) and a format component
/// (matched by arrow datatype AND semantic type) were found on the entity.
#[derive(Clone, Debug)]
pub struct BufferAndFormatMatch {
    /// The buffer slot on the visualizer that needs to be mapped.
    pub buffer_target: ComponentIdentifier,

    /// The format slot on the visualizer that needs to be mapped.
    pub format_target: ComponentIdentifier,

    /// All entity components whose arrow datatype matched the buffer's expected type.
    ///
    /// Guaranteed to have at least one entry.
    pub buffer_matches: IntMap<ComponentIdentifier, DatatypeMatch>,

    /// The entity components that matched the format (by arrow datatype AND semantic type).
    ///
    /// Guaranteed to have at least one entry.
    pub format_matches: IntSet<ComponentIdentifier>,
}

impl re_byte_size::SizeBytes for SingleRequiredComponentMatch {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            target_component,
            matches,
        } = self;
        target_component.heap_size_bytes() + matches.heap_size_bytes()
    }
}

impl re_byte_size::SizeBytes for BufferAndFormatMatch {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            buffer_target,
            format_target,
            buffer_matches,
            format_matches,
        } = self;
        buffer_target.heap_size_bytes()
            + format_target.heap_size_bytes()
            + buffer_matches.heap_size_bytes()
            + format_matches.heap_size_bytes()
    }
}

/// Describes why a given entity was marked as visualizable.
#[derive(Clone, Debug)]
pub enum VisualizableReason {
    /// The entity is visualizable because all entities are visualizable for this type.
    Always,

    /// [`crate::VisualizabilityConstraints::AnyBuiltinComponent`] matched for this entity.
    ExactMatchAny,

    /// See [`SingleRequiredComponentMatch`].
    SingleRequiredComponentMatch(SingleRequiredComponentMatch),

    /// See [`BufferAndFormatMatch`].
    BufferAndFormatMatch(BufferAndFormatMatch),
}

impl re_byte_size::SizeBytes for VisualizableReason {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Always | Self::ExactMatchAny => 0,
            Self::SingleRequiredComponentMatch(m) => m.heap_size_bytes(),
            Self::BufferAndFormatMatch(m) => m.heap_size_bytes(),
        }
    }
}

impl VisualizableReason {
    /// Returns true if this match reason is a perfect match for the given component identifier.
    pub fn full_native_match(&self, component_identifier: ComponentIdentifier) -> bool {
        match self {
            Self::Always | Self::ExactMatchAny => true,

            Self::SingleRequiredComponentMatch(m) => m
                .matches
                .get(&component_identifier)
                .map(|info| matches!(info, DatatypeMatch::NativeSemantics { .. }))
                .unwrap_or(false),

            Self::BufferAndFormatMatch(m) => {
                // Format is always native by construction (semantic match required).
                if m.format_matches.contains(&component_identifier) {
                    return true;
                }
                // Check if the buffer has a native semantic match.
                m.buffer_matches
                    .get(&component_identifier)
                    .map(|info| matches!(info, DatatypeMatch::NativeSemantics { .. }))
                    .unwrap_or(false)
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

impl re_byte_size::SizeBytes for VisualizableEntities {
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

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

impl<T> PerVisualizerType<T> {
    /// Convert from `PerVisualizerType<T>` to `PerVisualizerType<&T>`.
    #[inline]
    pub fn as_ref(&self) -> PerVisualizerType<&T> {
        PerVisualizerType(self.0.iter().map(|(&k, v)| (k, v)).collect())
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
