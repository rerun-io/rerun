use re_data_store::EntityProperties;
use re_log_types::EntityPath;
use re_query::{query_archetype_with_history, ArchetypeView, QueryError};
use re_renderer::DepthOffset;
use re_types::Archetype;
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewClass, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext,
};

use crate::{
    contexts::{
        AnnotationSceneContext, EntityDepthOffsets, PrimitiveCounter, SharedRenderBuilders,
        SpatialSceneEntityContext, TransformContext,
    },
    SpatialSpaceView3D,
};

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
#[allow(dead_code)]
pub fn process_archetype_views<'a, System: IdentifiedViewSystem, A, const N: usize, F>(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    default_depth_offset: DepthOffset,
    mut fun: F,
) -> Result<(), SpaceViewSystemExecutionError>
where
    A: Archetype + 'a,
    F: FnMut(
        &ViewerContext<'_>,
        &EntityPath,
        &EntityProperties,
        ArchetypeView<A>,
        &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError>,
{
    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;
    let shared_render_builders = view_ctx.get::<SharedRenderBuilders>()?;
    let counter = view_ctx.get::<PrimitiveCounter>()?;

    for data_result in query.iter_visible_data_results(System::identifier()) {
        // The transform that considers pinholes only makes sense if this is a 3D space-view
        let world_from_entity =
            if view_ctx.space_view_class_identifier() == SpatialSpaceView3D.identifier() {
                transforms.reference_from_entity(&data_result.entity_path)
            } else {
                transforms.reference_from_entity_ignoring_pinhole(
                    &data_result.entity_path,
                    ctx.store_db.store(),
                    &query.latest_at_query(),
                )
            };

        let Some(world_from_entity) = world_from_entity else {
            continue;
        };
        let entity_context = SpatialSceneEntityContext {
            world_from_entity,
            depth_offset: *depth_offsets
                .per_entity
                .get(&data_result.entity_path.hash())
                .unwrap_or(&default_depth_offset),
            annotations: annotations.0.find(&data_result.entity_path),
            shared_render_builders,
            highlight: query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash()),
            space_view_class_identifier: view_ctx.space_view_class_identifier(),
        };

        match query_archetype_with_history::<A, N>(
            ctx.store_db.store(),
            &query.timeline,
            &query.latest_at,
            &data_result.accumulated_properties().visible_history,
            &data_result.entity_path,
        )
        .and_then(|entity_views| {
            for ent_view in entity_views {
                counter.num_primitives.fetch_add(
                    ent_view.num_instances(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                fun(
                    ctx,
                    &data_result.entity_path,
                    data_result.accumulated_properties(),
                    ent_view,
                    &entity_context,
                )?;
            }
            Ok(())
        }) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => {}
            Err(err) => {
                re_log::error_once!(
                    "Unexpected error querying {:?}: {err}",
                    &data_result.entity_path
                );
            }
        }
    }

    Ok(())
}
