//! Various strongly typed sets of entities to express intent and avoid mistakes.

use nohash_hasher::{IntMap, IntSet};
use re_chunk::ComponentIdentifier;
use re_log_types::EntityPath;
use re_types_core::ViewClassIdentifier;
use vec1::smallvec_v1::SmallVec1;

use crate::ViewSystemIdentifier;

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
    // TODO(grtlr, andreas): Should primitive-castables live in the same struct? Probably only relevant if we care about conversions outside of the actual querysite.
    DatatypeMatchAny {
        components: SmallVec1<[ComponentIdentifier; 1]>,
    },
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
/// TODO(RR-3305): should this always be per visualizer instruction id rather than per visualizer type? depends on the usecase probably. Best do audit all usages of this!
///
/// Careful, if you're in the context of a view, this may contain visualizers that aren't relevant to the current view.
/// Refer to [`PerVisualizerInViewClass`] for a collection that is limited to visualizers active for a given view.
#[derive(Debug)]
pub struct PerVisualizer<T>(pub IntMap<ViewSystemIdentifier, T>);

impl<T> std::ops::Deref for PerVisualizer<T> {
    type Target = IntMap<ViewSystemIdentifier, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Clone> Clone for PerVisualizer<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Manual default impl, otherwise T: Default would be required.
impl<T> Default for PerVisualizer<T> {
    fn default() -> Self {
        Self(IntMap::default())
    }
}

/// Like [`PerVisualizer`], but ensured that all visualizers are relevant for the given view class.
#[derive(Debug)]
pub struct PerVisualizerInViewClass<T> {
    /// View for which this list is filtered down.
    ///
    /// Most of the time we don't actually need this field but it is useful for debugging
    /// and ensuring that [`Self::per_visualizer`] is scoped down to this view.
    pub view_class_identifier: ViewClassIdentifier,

    /// Items per visualizer system.
    pub per_visualizer: IntMap<ViewSystemIdentifier, T>,
}

impl<T> PerVisualizerInViewClass<T> {
    pub fn empty(view_class_identifier: ViewClassIdentifier) -> Self {
        Self {
            view_class_identifier,
            per_visualizer: Default::default(),
        }
    }
}

impl<T> std::ops::Deref for PerVisualizerInViewClass<T> {
    type Target = IntMap<ViewSystemIdentifier, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.per_visualizer
    }
}
