mod annotation_context;
mod depth_offsets;
mod non_interactive_entities;
mod shared_render_builders;
mod transform_context;

use std::sync::atomic::AtomicUsize;

pub use annotation_context::AnnotationSceneContext;
pub use depth_offsets::EntityDepthOffsets;
pub use shared_render_builders::SharedRenderBuilders;
pub use transform_context::TransformContext;

// -----------------------------------------------------------------------------

use re_log_types::EntityPath;
use re_renderer::DepthOffset;
use re_viewer_context::{Annotations, SceneContext};

use self::non_interactive_entities::NonInteractiveEntities;

#[derive(Default)]
pub struct SpatialSceneContext {
    pub transforms: TransformContext,
    pub depth_offsets: EntityDepthOffsets,
    pub annotations: AnnotationSceneContext,
    pub shared_render_builders: SharedRenderBuilders,
    pub non_interactive_entities: NonInteractiveEntities,

    pub num_primitives: AtomicUsize,
    pub num_3d_primitives: AtomicUsize,
}

impl SceneContext for SpatialSceneContext {
    fn vec_mut(&mut self) -> Vec<&mut dyn re_viewer_context::SceneContextPart> {
        let Self {
            transforms,
            depth_offsets,
            annotations,
            shared_render_builders,
            non_interactive_entities,
            num_3d_primitives: _,
            num_primitives: _,
        } = self;
        vec![
            transforms,
            depth_offsets,
            annotations,
            shared_render_builders,
            non_interactive_entities,
        ]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl SpatialSceneContext {
    pub fn lookup_entity_context<'a>(
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
            shared_render_builders: &self.shared_render_builders,
            highlight: highlights.entity_outline_mask(ent_path.hash()),
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
}
