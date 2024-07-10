mod annotation_context;
mod depth_offsets;
mod transform_context;

pub use annotation_context::AnnotationSceneContext;
pub use depth_offsets::EntityDepthOffsets;
use re_types::SpaceViewClassIdentifier;
pub use transform_context::TransformContext;

// -----------------------------------------------------------------------------

use re_renderer::DepthOffset;
use re_viewer_context::{Annotations, SpaceViewClassRegistryError};

/// Context objects for a single entity in a spatial scene.
pub struct SpatialSceneEntityContext<'a> {
    pub world_from_entity: glam::Affine3A,
    pub depth_offset: DepthOffset,
    pub annotations: std::sync::Arc<Annotations>,

    pub highlight: &'a re_viewer_context::SpaceViewOutlineMasks, // Not part of the context, but convenient to have here.
    pub space_view_class_identifier: SpaceViewClassIdentifier,
}

pub fn register_spatial_contexts(
    system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    system_registry.register_context_system::<TransformContext>()?;
    system_registry.register_context_system::<EntityDepthOffsets>()?;
    system_registry.register_context_system::<AnnotationSceneContext>()?;
    Ok(())
}
