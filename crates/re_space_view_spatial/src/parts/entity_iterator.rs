use re_log_types::{EntityPath, LegacyComponent};
use re_query::{
    query_archetype_with_history, query_primary_with_history, ArchetypeView, EntityView, QueryError,
};
use re_renderer::DepthOffset;
use re_types::{Archetype, ComponentName};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
};

use crate::contexts::{
    AnnotationSceneContext, EntityDepthOffsets, PrimitiveCounter, SharedRenderBuilders,
    SpatialSceneEntityContext, TransformContext,
};

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
pub fn process_entity_views<'a, System: NamedViewSystem, Primary, const N: usize, F>(
    ctx: &mut ViewerContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    default_depth_offset: DepthOffset,
    archetype: Vec<ComponentName>,
    mut fun: F,
) -> Result<(), SpaceViewSystemExecutionError>
where
    Primary: LegacyComponent + re_types::Component + 'a,
    F: FnMut(
        &mut ViewerContext<'_>,
        &EntityPath,
        EntityView<Primary>,
        &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError>,
{
    let archetype: [ComponentName; N] = match archetype.try_into() {
        Ok(archetype) => archetype,
        Err(archetype) => {
            re_log::error_once!(
                "Archetype {:?} has wrong number of elements, expected {N}",
                archetype
            );
            return Ok(());
        }
    };

    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;
    let shared_render_builders = view_ctx.get::<SharedRenderBuilders>()?;
    let counter = view_ctx.get::<PrimitiveCounter>()?;

    for (ent_path, props) in query.iter_entities_for_system(System::name()) {
        let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
            continue;
        };
        let entity_context = SpatialSceneEntityContext {
            world_from_obj,
            depth_offset: *depth_offsets
                .per_entity
                .get(&ent_path.hash())
                .unwrap_or(&default_depth_offset),
            annotations: annotations.0.find(ent_path),
            shared_render_builders,
            highlight: query.highlights.entity_outline_mask(ent_path.hash()),
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
                counter.num_primitives.fetch_add(
                    ent_view.num_instances(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                fun(ctx, ent_path, ent_view, &entity_context)?;
            }
            Ok(())
        }) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => {}
            Err(err) => {
                re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
            }
        }
    }

    Ok(())
}

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
#[allow(dead_code)]
pub fn process_archetype_views<'a, System: NamedViewSystem, A, const N: usize, F>(
    ctx: &mut ViewerContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    default_depth_offset: DepthOffset,
    mut fun: F,
) -> Result<(), SpaceViewSystemExecutionError>
where
    A: Archetype + 'a,
    F: FnMut(
        &mut ViewerContext<'_>,
        &EntityPath,
        ArchetypeView<A>,
        &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError>,
{
    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;
    let shared_render_builders = view_ctx.get::<SharedRenderBuilders>()?;
    let counter = view_ctx.get::<PrimitiveCounter>()?;

    for (ent_path, props) in query.iter_entities_for_system(System::name()) {
        let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
            continue;
        };
        let entity_context = SpatialSceneEntityContext {
            world_from_obj,
            depth_offset: *depth_offsets
                .per_entity
                .get(&ent_path.hash())
                .unwrap_or(&default_depth_offset),
            annotations: annotations.0.find(ent_path),
            shared_render_builders,
            highlight: query.highlights.entity_outline_mask(ent_path.hash()),
        };

        match query_archetype_with_history::<A, N>(
            &ctx.store_db.entity_db.data_store,
            &query.timeline,
            &query.latest_at,
            &props.visible_history,
            ent_path,
        )
        .and_then(|entity_views| {
            for ent_view in entity_views {
                counter.num_primitives.fetch_add(
                    ent_view.num_instances(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                fun(ctx, ent_path, ent_view, &entity_context)?;
            }
            Ok(())
        }) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => {}
            Err(err) => {
                re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
            }
        }
    }

    Ok(())
}
