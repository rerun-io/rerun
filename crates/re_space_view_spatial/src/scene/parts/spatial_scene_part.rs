use re_components::Component;
use re_data_store::EntityPath;
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::DepthOffset;
use re_viewer_context::{ArchetypeDefinition, SceneQuery, SpaceViewHighlights, ViewerContext};

use crate::scene::{
    contexts::{SpatialSceneContext, SpatialSceneEntityContext},
    UiLabel,
};

/// Common data struct for all spatial scene elements.
pub struct SpatialScenePartData {
    pub ui_labels: Vec<UiLabel>,
    pub bounding_box: macaw::BoundingBox,
}

impl Default for SpatialScenePartData {
    fn default() -> Self {
        Self {
            ui_labels: Vec::new(),
            bounding_box: macaw::BoundingBox::nothing(),
        }
    }
}

pub fn for_each_entity_view<'a, Primary, const N: usize, F>(
    ctx: &mut ViewerContext<'_>,
    query: &SceneQuery<'_>,
    context: &SpatialSceneContext,
    highlights: &SpaceViewHighlights,
    default_depth_offset: DepthOffset,
    archetype: ArchetypeDefinition,
    mut fun: F,
) where
    Primary: Component + 'a,
    F: FnMut(
        &EntityPath,
        EntityView<Primary>,
        &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError>,
{
    let archetype = match archetype.try_into() {
        Ok(archetype) => archetype,
        Err(archetype) => {
            re_log::error_once!(
                "Archetype {:?} has wrong number of elements, expected {N}",
                archetype
            );
            return;
        }
    };

    for (ent_path, props) in query.iter_entities() {
        let Some(entity_context) = context.query(ent_path, highlights, default_depth_offset) else {
            continue;
        };

        match query_primary_with_history::<Primary, N>(
            &ctx.store_db.entity_db.data_store,
            &query.timeline,
            &query.latest_at,
            &props.visible_history,
            ent_path,
            archetype,
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
