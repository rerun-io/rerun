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
        AnnotationSceneContext, EntityDepthOffsets, PrimitiveCounter, SharedRenderBuilders,
        SpatialSceneEntityContext, TransformContext,
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
            shared_render_builders,
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
            cached: bool,
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
                (TimeInt, RowId),
                &[InstanceKey],
                $(&[$pov],)*
                $(&[Option<$comp>],)*
            ) -> ::re_query::Result<()>,
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "process_archetype",
                format!("cached={cached} arch={} pov={} comp={}", A::name(), $N, $M)
            );

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
                    shared_render_builders,
                    highlight: query
                        .highlights
                        .entity_outline_mask(data_result.entity_path.hash()),
                    space_view_class_identifier: view_ctx.space_view_class_identifier(),
                };

                ::re_query_cache::[<query_archetype_with_history_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    cached,
                    ctx.entity_db.store(),
                    &query.timeline,
                    &query.latest_at,
                    &data_result.accumulated_properties().visible_history,
                    &data_result.entity_path,
                    |(t, keys, $($pov,)+ $($comp,)*)| {
                        counter
                            .num_primitives
                            .fetch_add(keys.as_slice().len(), std::sync::atomic::Ordering::Relaxed);

                        if let Err(err) = f(
                            ctx,
                            &data_result.entity_path,
                            data_result.accumulated_properties(),
                            &entity_context,
                            t,
                            keys.as_slice(),
                            $($pov.as_slice(),)+
                            $($comp.as_slice(),)*
                        ) {
                            re_log::error_once!(
                                "Unexpected error querying {:?}: {err}",
                                &data_result.entity_path
                            );
                        }
                    }
                )?;
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
