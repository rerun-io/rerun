use itertools::Either;
use re_chunk_store::{LatestAtQuery, RangeQuery};
use re_log_types::{TimeInt, Timeline};
use re_space_view::{
    latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data, HybridResults,
};
use re_types::Archetype;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, QueryRange, SpaceViewClass, SpaceViewSystemExecutionError,
    ViewContext, ViewContextCollection, ViewQuery,
};

use crate::{
    contexts::{
        AnnotationSceneContext, EntityDepthOffsets, SpatialSceneEntityContext, TransformContext,
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

pub fn query_archetype_with_history<'a, A: Archetype>(
    ctx: &'a ViewContext<'a>,
    timeline: &Timeline,
    timeline_cursor: TimeInt,
    query_range: &QueryRange,
    data_result: &'a re_viewer_context::DataResult,
) -> HybridResults<'a> {
    match query_range {
        QueryRange::TimeRange(time_range) => {
            let range_query = RangeQuery::new(
                *timeline,
                re_log_types::ResolvedTimeRange::from_relative_time_range(
                    time_range,
                    timeline_cursor,
                ),
            );
            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                A::all_components().iter().copied(),
            );
            (range_query, results).into()
        }
        QueryRange::LatestAt => {
            let latest_query = LatestAtQuery::new(*timeline, timeline_cursor);
            let query_shadowed_defaults = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_query,
                data_result,
                A::all_components().iter().copied(),
                query_shadowed_defaults,
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
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    mut fun: F,
) -> Result<(), SpaceViewSystemExecutionError>
where
    A: Archetype,
    F: FnMut(
        &QueryContext<'_>,
        &SpatialSceneEntityContext<'_>,
        &HybridResults<'_>,
    ) -> Result<(), SpaceViewSystemExecutionError>,
{
    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;

    let latest_at = query.latest_at_query();

    let system_identifier = System::identifier();

    for data_result in query.iter_visible_data_results(ctx, system_identifier) {
        // The transform that considers pinholes only makes sense if this is a 3D space-view
        let world_from_entity =
            if view_ctx.space_view_class_identifier() == SpatialSpaceView3D::identifier() {
                transforms.reference_from_entity(&data_result.entity_path)
            } else {
                transforms.reference_from_entity_ignoring_pinhole(
                    &data_result.entity_path,
                    ctx.recording(),
                    &latest_at,
                )
            };

        let Some(world_from_entity) = world_from_entity else {
            continue;
        };
        let depth_offset_key = (system_identifier, data_result.entity_path.hash());
        let entity_context = SpatialSceneEntityContext {
            world_from_entity,
            depth_offset: depth_offsets
                .per_entity_and_visualizer
                .get(&depth_offset_key)
                .copied()
                .unwrap_or_default(),
            annotations: annotations.0.find(&data_result.entity_path),
            highlight: query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash()),
            space_view_class_identifier: view_ctx.space_view_class_identifier(),
        };

        let results = query_archetype_with_history::<A>(
            ctx,
            &query.timeline,
            query.latest_at,
            data_result.query_range(),
            data_result,
        );

        let mut query_ctx = ctx.query_context(data_result, &latest_at);
        query_ctx.archetype_name = Some(A::name());

        {
            re_tracing::profile_scope!(format!("{}", data_result.entity_path));
            fun(&query_ctx, &entity_context, &results)?;
        }
    }

    Ok(())
}
