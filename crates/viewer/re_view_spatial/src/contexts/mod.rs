mod depth_offsets;
mod transform_tree_context;

pub use depth_offsets::EntityDepthOffsets;
// -----------------------------------------------------------------------------
use re_renderer::DepthOffset;
use re_sdk_types::ViewClassIdentifier;
use re_view::AnnotationSceneContext;
use re_viewer_context::{Annotations, ViewClassRegistryError};
pub use transform_tree_context::{TransformInfo, TransformTreeContext};

/// Context objects for a single visualizer instruction in a spatial scene.
pub struct SpatialSceneVisualizerInstructionContext<'a> {
    pub instruction_id: &'a re_sdk_types::blueprint::components::VisualizerInstructionId,

    pub transform_info: &'a TransformInfo,
    pub depth_offset: DepthOffset,
    pub annotations: std::sync::Arc<Annotations>,

    pub highlight: &'a re_viewer_context::ViewOutlineMasks, // Not part of the context, but convenient to have here.
    pub view_class_identifier: ViewClassIdentifier,

    pub output: &'a mut re_viewer_context::VisualizerExecutionOutput,
}

impl SpatialSceneVisualizerInstructionContext<'_> {
    /// Convenience method to report an error for this visualizer instruction.
    pub fn report_error(&mut self, error: impl Into<String>) {
        self.output.report_error_for(*self.instruction_id, error);
    }
}

pub fn register_spatial_contexts(
    system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
) -> Result<(), ViewClassRegistryError> {
    system_registry.register_context_system::<TransformTreeContext>()?;
    system_registry.register_context_system::<EntityDepthOffsets>()?;

    system_registry.register_context_system::<AnnotationSceneContext>()?;
    re_viewer_context::AnnotationContextStoreSubscriber::subscription_handle(); // Needed by `AnnotationSceneContext`

    Ok(())
}
