use itertools::Either;
use re_data_store::{LatestAtQuery, RangeQuery};
use re_entity_db::{EntityDb, EntityProperties};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_query::Results;
use re_renderer::DepthOffset;
use re_types::Archetype;
use re_viewer_context::{
    IdentifiedViewSystem, QueryRange, SpaceViewClass, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{
        AnnotationSceneContext, EntityDepthOffsets, PrimitiveCounter, SpatialSceneEntityContext,
        TransformContext,
    },
    SpatialSpaceView3D,
};

// ---

/// Clamp the latest value in `values` in order to reach a length of `clamped_len`.
///
/// Returns an empty iterator if values is empty.
#[inline]
pub fn clamped<T>(values: &[T], clamped_len: usize) -> impl Iterator<Item = &T> {
    let Some(last) = values.last() else {
        return Either::Left(std::iter::empty());
    };

    Either::Right(
        values
            .iter()
            .chain(std::iter::repeat(last))
            .take(clamped_len),
    )
}

// --- Cached APIs ---

pub fn query_archetype_with_history<A: Archetype>(
    entity_db: &EntityDb,
    timeline: &Timeline,
    timeline_cursor: TimeInt,
    query_range: &QueryRange,
    entity_path: &EntityPath,
) -> Results {
    let store = entity_db.store();
    let caches = entity_db.query_caches();

    match query_range {
        QueryRange::TimeRange(time_range) => {
            let range_query = RangeQuery::new(
                *timeline,
                re_log_types::TimeRange::from_visible_time_range(time_range, timeline_cursor),
            );
            let results = caches.range(
                store,
                &range_query,
                entity_path,
                A::all_components().iter().copied(),
            );
            (range_query, results).into()
        }
        QueryRange::LatestAt => {
            let latest_query = LatestAtQuery::new(*timeline, timeline_cursor);
            let results = caches.latest_at(
                store,
                &latest_query,
                entity_path,
                A::all_components().iter().copied(),
            );
            (latest_query, results).into()
        }
    }
}

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed along an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
pub fn process_archetype<System: IdentifiedViewSystem, A, F>(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    default_depth_offset: DepthOffset,
    mut fun: F,
) -> Result<(), SpaceViewSystemExecutionError>
where
    A: Archetype,
    F: FnMut(
        &ViewerContext<'_>,
        &EntityPath,
        &EntityProperties,
        &SpatialSceneEntityContext<'_>,
        &Results,
    ) -> Result<(), SpaceViewSystemExecutionError>,
{
    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;
    let counter = view_ctx.get::<PrimitiveCounter>()?;

    for data_result in query.iter_visible_data_results(ctx, System::identifier()) {
        // The transform that considers pinholes only makes sense if this is a 3D space-view
        let world_from_entity =
            if view_ctx.space_view_class_identifier() == SpatialSpaceView3D::identifier() {
                transforms.reference_from_entity(&data_result.entity_path)
            } else {
                transforms.reference_from_entity_ignoring_pinhole(
                    &data_result.entity_path,
                    ctx.recording(),
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
            highlight: query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash()),
            space_view_class_identifier: view_ctx.space_view_class_identifier(),
        };

        let results = query_archetype_with_history::<A>(
            ctx.recording(),
            &query.timeline,
            query.latest_at,
            data_result.query_range(),
            &data_result.entity_path,
        );

        // NOTE: We used to compute the number of primitives across the entire scene here, but that
        // seems a bit excessive now that we have promises, as that would require resolving all
        // promises across all entities right here right now.
        // Also the count doesn't seem to be used for much anyhow.
        //
        // We'll see how things evolve.
        _ = counter;

        fun(
            ctx,
            &data_result.entity_path,
            data_result.accumulated_properties(),
            &entity_context,
            &results,
        )?;
    }

    Ok(())
}
