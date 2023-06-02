use re_components::Component;
use re_data_store::EntityPath;
use re_log_types::ComponentName;
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::DepthOffset;
use re_viewer_context::{SceneElementImpl, SceneQuery, SpaceViewHighlights, ViewerContext};

use super::{
    contexts::{SpatialSceneContext, SpatialSceneEntityContext},
    UiLabel,
};

/// Common data struct for all spatial scene elements.
pub struct SpatialSceneElementData {
    pub ui_labels: Vec<UiLabel>,
    pub bounding_box: macaw::BoundingBox,
}

impl Default for SpatialSceneElementData {
    fn default() -> Self {
        Self {
            ui_labels: Vec::new(),
            bounding_box: macaw::BoundingBox::nothing(),
        }
    }
}

pub trait SpatialSceneElement<const N: usize>: std::any::Any {
    type Primary: Component + 'static;

    fn archetype() -> [ComponentName; N];

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData>;

    fn data(&self) -> &SpatialSceneElementData;

    fn for_each_entity_view<F>(
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
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
            let Some(entity_context) = context.query(ent_path, highlights, default_depth_offset) else {
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
                for ent_view in entity_views {
                    context.num_primitives.fetch_add(
                        ent_view.num_instances(),
                        std::sync::atomic::Ordering::Relaxed,
                    );

                    fun(ent_path, ent_view, &entity_context)?;
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

impl<const N: usize, T: SpatialSceneElement<N>> SceneElementImpl
    for SpatialSceneElementWrapper<N, T>
{
    type SpaceViewState = re_space_view::EmptySpaceViewState;
    type SceneContext = SpatialSceneContext;

    fn archetype(&self) -> re_viewer_context::ArchetypeDefinition {
        debug_assert!(N > 0);
        T::archetype().to_vec().try_into().unwrap()
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &Self::SpaceViewState,
        context: &Self::SceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        self.0.populate(ctx, query, context, highlights)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.data())
    }
}
