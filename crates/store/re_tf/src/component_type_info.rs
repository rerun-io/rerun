use std::sync::OnceLock;

use nohash_hasher::IntSet;
use re_types::{Archetype as _, Component as _, ComponentType, archetypes, components};

/// Lists all Rerun components types that are relevant for the transform cache.
pub struct TransformComponentTypeInfo {
    /// All components of [`archetypes::Transform3D`]
    pub transform: IntSet<ComponentType>,

    /// All components of [`archetypes::InstancePoses3D`]
    pub pose: IntSet<ComponentType>,

    /// All components related to pinholes (i.e. [`components::PinholeProjection`] and [`components::ViewCoordinates`]).
    pub pinhole: IntSet<ComponentType>,
}

impl Default for TransformComponentTypeInfo {
    fn default() -> Self {
        Self {
            transform: archetypes::Transform3D::all_components()
                .iter()
                .filter_map(|descr| descr.component_type)
                .collect(),
            pose: archetypes::InstancePoses3D::all_components()
                .iter()
                .filter_map(|descr| descr.component_type)
                .collect(),
            pinhole: [
                components::PinholeProjection::name(),
                components::ViewCoordinates::name(),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl TransformComponentTypeInfo {
    /// Retrieves global cached instance.
    pub fn get() -> &'static Self {
        static ONCE: OnceLock<TransformComponentTypeInfo> = OnceLock::new();
        ONCE.get_or_init(Default::default)
    }
}
