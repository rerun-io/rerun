use re_entity_db::EntityProperties;
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
        AnnotationSceneContext, EntityDepthOffsets, PrimitiveCounter, SpatialSceneEntityContext,
        TransformContext,
    },
    SpatialSpaceView3D,
};

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed a long an [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
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
    let counter = view_ctx.get::<PrimitiveCounter>()?;

    for data_result in query.iter_visible_data_results(System::identifier()) {
        // The transform that considers pinholes only makes sense if this is a 3D space-view
        let world_from_entity =
            if view_ctx.space_view_class_identifier() == SpatialSpaceView3D.identifier() {
                transforms.reference_from_entity(&data_result.entity_path)
            } else {
                transforms.reference_from_entity_ignoring_pinhole(
                    &data_result.entity_path,
                    ctx.entity_db.store(),
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

        match query_archetype_with_history::<A, N>(
            ctx.entity_db.store(),
            &query.timeline,
            &query.latest_at,
            &data_result.accumulated_properties().visible_history,
            &data_result.entity_path,
        )
        .and_then(|arch_views| {
            for arch_view in arch_views {
                counter.num_primitives.fetch_add(
                    arch_view.num_instances(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                fun(
                    ctx,
                    &data_result.entity_path,
                    data_result.accumulated_properties(),
                    arch_view,
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

use re_query_cache::external::{paste::paste, seq_macro::seq};

macro_rules! impl_process_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`process_archetype_views] for `" $N "` point-of-view components"]
        #[doc = "and `" $M "` optional components."]
        #[allow(non_snake_case, dead_code)]
        pub fn [<process_archetype_pov$N _comp$M>]<'a, S, A, $($pov,)+ $($comp,)* F>(
            ctx: &ViewerContext<'_>,
            query: &ViewQuery<'_>,
            view_ctx: &ViewContextCollection,
            default_depth_offset: DepthOffset,
            mut f: F,
        ) -> Result<(), SpaceViewSystemExecutionError>
        where
            S: IdentifiedViewSystem,
            A: Archetype + 'a,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
            F: FnMut(
                &ViewerContext<'_>,
                &EntityPath,
                &EntityProperties,
                &SpatialSceneEntityContext<'_>,
                (Option<TimeInt>, RowId),
                &[InstanceKey],
                $(&[$pov],)*
                $(Option<&[Option<$comp>]>,)*
            ) -> Result<(), SpaceViewSystemExecutionError>,
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "process_archetype",
                format!("arch={} pov={} comp={}", A::name(), $N, $M)
            );

            let transforms = view_ctx.get::<TransformContext>()?;
            let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
            let annotations = view_ctx.get::<AnnotationSceneContext>()?;
            let counter = view_ctx.get::<PrimitiveCounter>()?;

            for data_result in query.iter_visible_data_results(S::identifier()) {
                // The transform that considers pinholes only makes sense if this is a 3D space-view
                let world_from_entity = if view_ctx.space_view_class_identifier() == SpatialSpaceView3D.identifier() {
                    transforms.reference_from_entity(&data_result.entity_path)
                } else {
                    transforms.reference_from_entity_ignoring_pinhole(
                        &data_result.entity_path,
                        ctx.entity_db.store(),
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

                match ctx.entity_db.query_caches().[<query_archetype_with_history_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    ctx.entity_db.store(),
                    &query.timeline,
                    &query.latest_at,
                    &data_result.accumulated_properties().visible_history,
                    &data_result.entity_path,
                    |(t, keys, $($pov,)+ $($comp,)*)| {
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
                            $($pov,)+
                            $($comp.as_deref(),)*
                        ) {
                            re_log::error_once!(
                                "Unexpected error querying {:?}: {err}",
                                &data_result.entity_path
                            );
                        };
                    }
                ) {
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
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_process_archetype!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_process_archetype!(for N=1, M=NUM_COMP);
});

/// Count the number of primary instances for a given archetype query that should be displayed.
///
/// Returned value might be conservative and some of the instances may not be displayable after all,
/// e.g. due to invalid transformation etc.
pub fn count_instances_in_archetype_views<
    System: IdentifiedViewSystem,
    A: Archetype,
    const N: usize,
>(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
) -> usize {
    assert_eq!(A::all_components().len(), N);

    // TODO(andreas): Use cached code path for this.
    // This is right now a bit harder to do and requires knowing all queried components.
    // The only thing we really want to pass here are the POV components.

    re_tracing::profile_function!();

    let mut num_instances = 0;

    for data_result in query.iter_visible_data_results(System::identifier()) {
        match query_archetype_with_history::<A, N>(
            ctx.entity_db.store(),
            &query.timeline,
            &query.latest_at,
            &data_result.accumulated_properties().visible_history,
            &data_result.entity_path,
        )
        .map(|arch_views| {
            for arch_view in arch_views {
                num_instances += arch_view.num_instances();
            }
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

    num_instances
}
