mod annotation_context;
mod depth_offsets;
mod non_interactive_entities;
mod transform_context;

use std::sync::atomic::AtomicUsize;

pub use annotation_context::AnnotationSceneContext;
pub use depth_offsets::EntityDepthOffsets;
pub use non_interactive_entities::NonInteractiveEntities;
pub use transform_context::TransformContext;

// -----------------------------------------------------------------------------

use re_renderer::DepthOffset;
use re_viewer_context::{
    Annotations, IdentifiedViewSystem, SpaceViewClassIdentifier, SpaceViewClassRegistryError,
    ViewContextSystem,
};

/// Context objects for a single entity in a spatial scene.
pub struct SpatialSceneEntityContext<'a> {
    pub world_from_entity: glam::Affine3A,
    pub depth_offset: DepthOffset,
    pub annotations: std::sync::Arc<Annotations>,

    pub highlight: &'a re_viewer_context::SpaceViewOutlineMasks, // Not part of the context, but convenient to have here.
    pub space_view_class_identifier: SpaceViewClassIdentifier,
}

#[derive(Default)]
pub struct PrimitiveCounter {
    pub num_primitives: AtomicUsize,
}

impl IdentifiedViewSystem for PrimitiveCounter {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "PrimitiveCounter".into()
    }
}

impl ViewContextSystem for PrimitiveCounter {
    fn compatible_component_sets(&self) -> Vec<re_types::ComponentNameSet> {
        Vec::new()
    }

    fn execute(
        &mut self,
        _ctx: &re_viewer_context::ViewerContext<'_>,
        _query: &re_viewer_context::ViewQuery<'_>,
    ) {
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn register_spatial_contexts(
    system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
) -> Result<(), SpaceViewClassRegistryError> {
    system_registry.register_context_system::<TransformContext>()?;
    system_registry.register_context_system::<EntityDepthOffsets>()?;
    system_registry.register_context_system::<AnnotationSceneContext>()?;
    system_registry.register_context_system::<NonInteractiveEntities>()?;
    system_registry.register_context_system::<PrimitiveCounter>()?;
    Ok(())
}
