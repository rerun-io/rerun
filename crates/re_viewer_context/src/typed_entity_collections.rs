//! Various strongly typed sets of entities to express intent and avoid mistakes.

use nohash_hasher::{IntMap, IntSet};
use re_log_types::{EntityPath, EntityPathHash};

use crate::ViewSystemIdentifier;

/// List of entities that are *applicable* to a given visualizer.
///
/// An entity is applicable if it at any point in time on any timeline has all required components.
#[derive(Default, Clone)]
pub struct VisualizerApplicableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for VisualizerApplicableEntities {
    type Target = IntSet<EntityPath>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of entities that can be visualized by a concrete visualizer.
///
/// This is a subset of [`VisualizerApplicableEntities`] and differs on a
/// per space view instance base.
#[derive(Default)]
pub struct VisualizableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for VisualizableEntities {
    type Target = IntSet<EntityPath>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of entities that match the indicator components of a visualizer.
///
/// In order to be a match the entity must have at some point in time on any timeline had any of the indicator components.
#[derive(Default, Clone)]
pub struct IndicatorMatchingEntities(pub IntSet<EntityPathHash>);

impl std::ops::Deref for IndicatorMatchingEntities {
    type Target = IntSet<EntityPathHash>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// -----------

/// List of entities that are applicable to each visualizer.
///
/// See [`IndicatorMatchingEntities`].
#[derive(Default)]
pub struct IndicatorMatchingEntitiesPerVisualizer(
    pub IntMap<ViewSystemIdentifier, IndicatorMatchingEntities>,
);

impl std::ops::Deref for IndicatorMatchingEntitiesPerVisualizer {
    type Target = IntMap<ViewSystemIdentifier, IndicatorMatchingEntities>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of entities that are applicable to each visualizer.
///
/// See [`VisualizerApplicableEntities`].
pub struct ApplicableEntitiesPerVisualizer(
    pub IntMap<ViewSystemIdentifier, VisualizerApplicableEntities>,
);

impl std::ops::Deref for ApplicableEntitiesPerVisualizer {
    type Target = IntMap<ViewSystemIdentifier, VisualizerApplicableEntities>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// List of entities that can be visualized per visualizer.
///
/// See [`VisualizableEntities`].
#[derive(Default)]
pub struct VisualizableEntitiesPerVisualizer(
    pub IntMap<ViewSystemIdentifier, VisualizableEntities>,
);

impl std::ops::Deref for VisualizableEntitiesPerVisualizer {
    type Target = IntMap<ViewSystemIdentifier, VisualizableEntities>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
