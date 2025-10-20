use re_chunk_store::Chunk;
use re_types::{Component as _, ComponentType};

use crate::component_type_info::TransformComponentTypeInfo;

bitflags::bitflags! {
    /// Flags for the different kinds of independent transforms that the transform cache handles.
    #[derive(Debug, Clone, Copy)]
    pub struct TransformAspect: u8 {
        /// The entity defines one of more frame relationships, i.e. any non-style component of [`archetypes::Transform3D`].
        // TODO(RR-2511): Add other components here.
        const Frame = 1 << 0;

        /// The entity has instance poses, i.e. any non-style component of [`archetypes::InstancePoses3D`].
        const Pose = 1 << 1;

        /// The entity has a pinhole projection or view coordinates, i.e. either [`components::PinholeProjection`] or [`components::ViewCoordinates`].
        const PinholeOrViewCoordinates = 1 << 2;

        /// The entity has a clear component.
        const Clear = 1 << 3;
    }
}

impl TransformAspect {
    /// Converts a component type to a transform aspect.
    pub fn from_component_type(component_type: ComponentType) -> Self {
        let component_info = TransformComponentTypeInfo::get();

        if component_info.transform.contains(&component_type) {
            Self::Frame
        } else if component_info.pose.contains(&component_type) {
            Self::Pose
        } else if component_info.pinhole.contains(&component_type) {
            Self::PinholeOrViewCoordinates
        } else if component_type == re_types::components::ClearIsRecursive::name() {
            Self::Clear
        } else {
            Self::empty()
        }
    }

    /// Collects the transform aspects a chunk covers.
    pub fn transform_aspects_of(chunk: &Chunk) -> Self {
        let mut aspects = Self::empty();
        for component_type in chunk
            .component_descriptors()
            .filter_map(|c| c.component_type)
        {
            aspects |= Self::from_component_type(component_type);
        }
        aspects
    }
}
