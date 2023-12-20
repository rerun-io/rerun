use re_data_store::EntityProperties;
use re_log_types::{EntityPath, RowId, TimeInt};
use re_query::{query_archetype_with_history, ArchetypeView, QueryError};
use re_renderer::DepthOffset;
use re_types::{components::InstanceKey, Archetype, Component};
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

// ---

macro_rules! impl_process_cached_archetype_views_rNoM {
    (impl $name:ident on top of $query_name:ident with required=[$($r:ident)+] optional=[$($o:ident)*]) => {
        /// Iterates through all entity views for a given archetype.
        ///
        /// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
        /// various useful information about an entity in the context of the current scene.
        // TODO
        #[allow(non_snake_case)]
        pub fn $name<'a, S, const N: usize, A, $($r,)+ $($o,)* F>(
            ctx: &ViewerContext<'_>,
            query: &ViewQuery<'_>,
            view_ctx: &ViewContextCollection,
            default_depth_offset: DepthOffset,
            mut f: F,
        ) -> Result<(), SpaceViewSystemExecutionError>
        where
            S: IdentifiedViewSystem,
            A: Archetype + 'a,
            $($r: Component + Send + Sync + 'static,)+
            $($o: Component + Send + Sync + 'static,)*
            F: FnMut(
                &ViewerContext<'_>,
                &EntityPath,
                &EntityProperties,
                &SpatialSceneEntityContext<'_>,
                &(TimeInt, RowId),
                &[InstanceKey],
                $(&[$r],)+
                $(&[Option<$o>],)*
            ) -> Result<(), QueryError>,
        {
            let transforms = view_ctx.get::<TransformContext>()?;
            let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
            let annotations = view_ctx.get::<AnnotationSceneContext>()?;
            let shared_render_builders = view_ctx.get::<SharedRenderBuilders>()?;
            let counter = view_ctx.get::<PrimitiveCounter>()?;

            for data_result in query.iter_visible_data_results(S::identifier()) {
                // The transform that considers pinholes only makes sense if this is a 3D space-view
                let world_from_entity = if view_ctx.space_view_class_identifier() == SpatialSpaceView3D.identifier() {
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

                ::re_query_cache::$query_name::<N, A, $($r,)+ $($o,)* _>(
                    ctx.store_db.store(),
                    &query.timeline,
                    &query.latest_at,
                    &data_result.accumulated_properties().visible_history,
                    &data_result.entity_path,
                    |it| {
                        for (t, keys, $($r,)+ $($o,)*) in it {
                            counter
                                .num_primitives
                                .fetch_add(keys.len(), std::sync::atomic::Ordering::Relaxed);

                            if let Err(err) = f(
                                ctx,
                                &data_result.entity_path,
                                data_result.accumulated_properties(),
                                &entity_context,
                                t,
                                keys,
                                $($r,)+
                                $($o,)*
                            ) {
                                re_log::error_once!(
                                    "Unexpected error querying {:?}: {err}",
                                    &data_result.entity_path
                                );
                            }
                        }
                    }
                )
            }

            Ok(())
        }
    };
    (impl $name:ident on top of $query_name:ident with required=[$($r:ident)+]) => {
        impl_process_cached_archetype_views_rNoM!(impl $name on top of $query_name with required=[$($r)+] optional=[]);
    };
}

impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1   on top of query_cached_archetype_with_history_r1
        with required=[R1]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o1 on top of query_cached_archetype_with_history_r1o1
        with required=[R1] optional=[O1]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o2 on top of query_cached_archetype_with_history_r1o2
        with required=[R1] optional=[O1 O2]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o3 on top of query_cached_archetype_with_history_r1o3
        with required=[R1] optional=[O1 O2 O3]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o4 on top of query_cached_archetype_with_history_r1o4
        with required=[R1] optional=[O1 O2 O3 O4]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o5 on top of query_cached_archetype_with_history_r1o5
        with required=[R1] optional=[O1 O2 O3 O4 O5]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o6 on top of query_cached_archetype_with_history_r1o6
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o7 on top of query_cached_archetype_with_history_r1o7
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o8 on top of query_cached_archetype_with_history_r1o8
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8]);
impl_process_cached_archetype_views_rNoM!(
    impl process_cached_archetype_views_r1o9 on top of query_cached_archetype_with_history_r1o9
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8 O9]);
