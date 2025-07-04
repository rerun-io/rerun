//! Various strongly typed sets of entities to express intent and avoid mistakes.

use nohash_hasher::{IntMap, IntSet};
use re_log_types::EntityPath;

use crate::ViewSystemIdentifier;

/// List of entities that are *maybe* visualizable with a given visualizer.
///
/// Note that this filter latches:
/// An entity is "maybe visualizable" if it at any point in time on any timeline has all required components.
///
/// We evaluate this filtering step entirely by store subscriber.
/// This in turn implies that this can *not* be influenced by individual view setups.
#[derive(Default, Clone, Debug)]
pub struct MaybeVisualizableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for MaybeVisualizableEntities {
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

/// List of entities that can be visualized at some point in time on any timeline
/// by a concrete visualizer in the context of a specific instantiated view.
///
/// It gets invalidated whenever any properties of the respective view instance
/// change, e.g. its origin.
/// TODO(andreas): Unclear if any of the view's configuring blueprint entities are included in this.
///
/// This is a subset of [`MaybeVisualizableEntities`] and may differs on a per view instance base!
#[derive(Default, Clone, Debug)]
pub struct VisualizableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for VisualizableEntities {
    type Target = IntSet<EntityPath>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Debug)]
pub struct PerVisualizer<T: Default>(pub IntMap<ViewSystemIdentifier, T>);

impl<T: Default> std::ops::Deref for PerVisualizer<T> {
    type Target = IntMap<ViewSystemIdentifier, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
