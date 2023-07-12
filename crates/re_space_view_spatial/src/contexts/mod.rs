mod annotation_context;
mod depth_offsets;
mod non_interactive_entities;
mod shared_render_builders;
mod transform_context;

use std::sync::atomic::AtomicUsize;

pub use annotation_context::AnnotationSceneContext;
pub use depth_offsets::EntityDepthOffsets;
pub use non_interactive_entities::NonInteractiveEntities;
pub use shared_render_builders::SharedRenderBuilders;
pub use transform_context::{pinhole_camera_view_coordinates, TransformContext};

// -----------------------------------------------------------------------------

use re_log_types::EntityPath;
use re_renderer::DepthOffset;
use re_viewer_context::{
    Annotations, SpaceViewSystemExecutionError, ViewContextCollection, ViewContextSystem,
};

///
pub struct SpatialViewContext<'a> {
    pub transforms: &'a TransformContext,
    pub depth_offsets: &'a EntityDepthOffsets,
    pub annotations: &'a AnnotationSceneContext,
    pub shared_render_builders: &'a SharedRenderBuilders,
    pub non_interactive_entities: &'a NonInteractiveEntities,
    pub counter: &'a PrimitiveCounter,
}

impl<'a> SpatialViewContext<'a> {
    pub fn new(context: &'a ViewContextCollection) -> Result<Self, SpaceViewSystemExecutionError> {
        Ok(Self {
            transforms: context.get::<TransformContext>()?,
            depth_offsets: context.get::<EntityDepthOffsets>()?,
            annotations: context.get::<AnnotationSceneContext>()?,
            shared_render_builders: context.get::<SharedRenderBuilders>()?,
            non_interactive_entities: context.get::<NonInteractiveEntities>()?,
            counter: context.get::<PrimitiveCounter>()?,
        })
    }

    pub fn lookup_entity_context(
        &'a self,
        ent_path: &EntityPath,
        highlights: &'a re_viewer_context::SpaceViewHighlights,
        default_depth_offset: DepthOffset,
    ) -> Option<SpatialSceneEntityContext<'a>> {
        Some(SpatialSceneEntityContext {
            world_from_obj: self.transforms.reference_from_entity(ent_path)?,
            depth_offset: *self
                .depth_offsets
                .per_entity
                .get(&ent_path.hash())
                .unwrap_or(&default_depth_offset),
            annotations: self.annotations.0.find(ent_path),
            shared_render_builders: self.shared_render_builders,
            highlight: highlights.entity_outline_mask(ent_path.hash()),
            ctx: self,
        })
    }
}

/// Context objects for a single entity in a spatial scene.
pub struct SpatialSceneEntityContext<'a> {
    pub world_from_obj: glam::Affine3A,
    pub depth_offset: DepthOffset,
    pub annotations: std::sync::Arc<Annotations>,
    pub shared_render_builders: &'a SharedRenderBuilders,
    pub highlight: &'a re_viewer_context::SpaceViewOutlineMasks, // Not part of the context, but convenient to have here.

    pub ctx: &'a SpatialViewContext<'a>,
}

#[derive(Default)]
pub struct PrimitiveCounter {
    pub num_primitives: AtomicUsize,
    pub num_3d_primitives: AtomicUsize,
}

impl ViewContextSystem for PrimitiveCounter {
    fn archetypes(&self) -> Vec<re_viewer_context::ArchetypeDefinition> {
        Vec::new()
    }

    fn execute(
        &mut self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        _query: &re_viewer_context::ViewQuery<'_>,
    ) {
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn register_contexts(system_registry: &mut re_viewer_context::SpaceViewSystemRegistry) {
    system_registry.register_context_system::<TransformContext>();
    system_registry.register_context_system::<EntityDepthOffsets>();
    system_registry.register_context_system::<AnnotationSceneContext>();
    system_registry.register_context_system::<SharedRenderBuilders>();
    system_registry.register_context_system::<NonInteractiveEntities>();
    system_registry.register_context_system::<PrimitiveCounter>();
}
