mod depth_offsets;
mod transform_context;

pub use depth_offsets::EntityDepthOffsets;
use re_types::ViewClassIdentifier;
use re_view::AnnotationSceneContext;
pub use transform_context::{TransformContext, TransformInfo, TwoDInThreeDTransformInfo};

// -----------------------------------------------------------------------------

use re_renderer::DepthOffset;
use re_viewer_context::{Annotations, ViewClassRegistryError};

/// Context objects for a single entity in a spatial scene.
pub struct SpatialSceneEntityContext<'a> {
    pub transform_info: &'a TransformInfo,
    pub depth_offset: DepthOffset,
    pub annotations: std::sync::Arc<Annotations>,

    pub highlight: &'a re_viewer_context::ViewOutlineMasks, // Not part of the context, but convenient to have here.
    pub space_view_class_identifier: ViewClassIdentifier,
}

pub fn register_spatial_contexts(
    system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
) -> Result<(), ViewClassRegistryError> {
    system_registry.register_context_system::<TransformContext>()?;
    system_registry.register_context_system::<EntityDepthOffsets>()?;
    system_registry.register_context_system::<AnnotationSceneContext>()?;
    Ok(())
}
