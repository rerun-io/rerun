use std::sync::Arc;

use re_components::Component;
use re_data_store::EntityPath;
use re_log_types::ComponentName;
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::DepthOffset;
use re_viewer_context::{
    Annotations, SceneContextCollection, SceneElement, SceneItemCollectionLookupError, SceneQuery,
    ViewerContext,
};
use re_viewer_context::{SpaceViewHighlights, SpaceViewOutlineMasks};

use crate::{scene::EntityDepthOffsets, TransformContext};

use super::contexts::{AnnotationSceneContext, SharedRenderBuilders};

/// Context objects for a single entity in a spatial scene.
pub struct SpatialSceneEntityContext<'a> {
    pub world_from_obj: glam::Affine3A,
    pub depth_offset: DepthOffset,
    pub annotations: Arc<Annotations>,
    pub highlight: &'a SpaceViewOutlineMasks,
    pub shared_render_builders: &'a SharedRenderBuilders,
}

/// Reference to all context objects of a spatial scene.
pub struct SpatialSceneContext<'a> {
    pub transforms: &'a TransformContext,
    pub depth_offsets: &'a EntityDepthOffsets,
    pub annotations: &'a AnnotationSceneContext,
    pub shared_render_builders: &'a SharedRenderBuilders,
    pub highlights: &'a SpaceViewHighlights, // Not part of the context collection, but convenient to have here.
}

impl<'a> SpatialSceneContext<'a> {
    pub fn new(
        contexts: &'a SceneContextCollection,
        highlights: &'a SpaceViewHighlights,
    ) -> Result<Self, SceneItemCollectionLookupError> {
        Ok(Self {
            transforms: contexts.get::<TransformContext>()?,
            depth_offsets: contexts.get::<EntityDepthOffsets>()?,
            annotations: contexts.get::<AnnotationSceneContext>()?,
            shared_render_builders: contexts.get::<SharedRenderBuilders>()?,
            highlights,
        })
    }

    fn query(
        &self,
        ent_path: &EntityPath,
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
            highlight: self.highlights.entity_outline_mask(ent_path.hash()),
        })
    }
}

pub trait SpatialSceneElement<const N: usize>: std::any::Any {
    type Primary: Component + 'static;

    fn archetype() -> [ComponentName; N];

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        context: SpatialSceneContext<'_>,
    ) -> Vec<re_renderer::QueueableDrawData>;

    fn for_each_entity_view<F>(
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        context: &SpatialSceneContext<'_>,
        default_depth_offset: DepthOffset,
        mut fun: F,
    ) where
        F: FnMut(
            &EntityPath,
            EntityView<Self::Primary>,
            &SpatialSceneEntityContext<'_>,
        ) -> Result<(), QueryError>,
    {
        for (ent_path, props) in query.iter_entities() {
            let Some(entity_context) = context.query(ent_path, default_depth_offset) else {
                continue;
            };

            match query_primary_with_history::<Self::Primary, N>(
                &ctx.store_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                Self::archetype(),
            )
            .and_then(|entity_views| {
                for entity_view in entity_views {
                    fun(ent_path, entity_view, &entity_context)?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }

    fn wrap(self) -> SpatialSceneElementWrapper<N, Self>
    where
        Self: Sized,
    {
        SpatialSceneElementWrapper(self)
    }
}

/// A wrapper for `SpatialSceneElement` that implements `SceneElement`.
///
/// Can't implement directly due to Rust limitations around higher kinded traits.
pub struct SpatialSceneElementWrapper<const N: usize, T: SpatialSceneElement<N>>(pub T);

impl<const N: usize, T: SpatialSceneElement<N>> SceneElement for SpatialSceneElementWrapper<N, T> {
    fn archetype(&self) -> re_viewer_context::ArchetypeDefinition {
        debug_assert!(N > 0);
        T::archetype().to_vec().try_into().unwrap()
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &dyn re_viewer_context::SpaceViewState,
        contexts: &re_viewer_context::SceneContextCollection,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        let contexts = match SpatialSceneContext::new(contexts, highlights) {
            Ok(ctx) => ctx,
            Err(err) => {
                re_log::error_once!("Failed to get required scene contexts: {err}");
                return Vec::new();
            }
        };
        self.0.populate(ctx, query, contexts)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        // Forwarding to the inner type allows to cast to the original implementor of SpatialSceneElement.
        &self.0
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        // Forwarding to the inner type allows to cast to the original implementor of SpatialSceneElement.
        &mut self.0
    }
}
