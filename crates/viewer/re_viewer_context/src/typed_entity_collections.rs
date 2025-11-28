//! Various strongly typed sets of entities to express intent and avoid mistakes.

use nohash_hasher::{IntMap, IntSet};
use re_log_types::EntityPath;
use re_types_core::ViewClassIdentifier;

use crate::ViewSystemIdentifier;

/// List of entities that are visualizable with a given visualizer.
///
/// Note that this filter latches:
/// An entity is marked visualizable if it at any point in time on any timeline has all required components.
///
/// We evaluate this filtering step entirely by store subscriber.
/// This in turn implies that this can *not* be influenced by individual view setups.
#[derive(Default, Clone, Debug)]
pub struct VisualizableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for VisualizableEntities {
    type Target = IntSet<EntityPath>;

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
