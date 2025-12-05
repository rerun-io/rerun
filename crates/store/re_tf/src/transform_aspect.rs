use re_chunk_store::Chunk;
use re_sdk_types::{Archetype as _, ArchetypeName, archetypes};

bitflags::bitflags! {
    /// Flags for the different kinds of independent transforms that the transform cache handles.
    #[derive(Debug, Clone, Copy)]
    pub struct TransformAspect: u8 {
        /// The entity defines one of more frame relationships, i.e. any non-style component of [`archetypes::Transform3D`].
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
    pub fn from_archetype(archetype: ArchetypeName) -> Self {
        if archetypes::Transform3D::name() == archetype {
            Self::Frame
        } else if archetypes::InstancePoses3D::name() == archetype {
            Self::Pose
        } else if archetypes::Pinhole::name() == archetype
            || archetypes::ViewCoordinates::name() == archetype
        {
            Self::PinholeOrViewCoordinates
        } else if archetypes::Clear::name() == archetype {
            Self::Clear
        } else {
            Self::empty()
        }
    }

    /// Collects the transform aspects a chunk covers.
    pub fn transform_aspects_of(chunk: &Chunk) -> Self {
        let mut aspects = Self::empty();
        for archetype in chunk.component_descriptors().filter_map(|c| c.archetype) {
            aspects |= Self::from_archetype(archetype);
        }
        aspects
    }
}
