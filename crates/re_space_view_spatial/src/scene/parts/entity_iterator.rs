use re_log_types::{Component, EntityPath};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::DepthOffset;
use re_viewer_context::{ArchetypeDefinition, SceneQuery, ViewerContext};

use crate::scene::contexts::{SpatialSceneContext, SpatialSceneEntityContext};

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
pub fn process_entity_views<'a, Primary, const N: usize, F>(
    ctx: &mut ViewerContext<'_>,
    query: &SceneQuery<'_>,
    context: &SpatialSceneContext,
    default_depth_offset: DepthOffset,
    archetype: ArchetypeDefinition,
    mut fun: F,
) where
    Primary: Component + 'a,
    F: FnMut(
        &mut ViewerContext<'_>,
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
        let Some(entity_context) = context.lookup_entity_context(ent_path, query.highlights, default_depth_offset) else {
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

                fun(ctx, ent_path, ent_view, &entity_context)?;
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
