//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod contexts;
mod eye;
mod heuristics;
mod instance_hash_conversions;
mod max_image_dimension_subscriber;
mod mesh_cache;
mod mesh_loader;
mod picking;
mod scene_bounding_boxes;
mod space_camera_3d;
mod spatial_topology;
mod ui;
mod ui_2d;
mod ui_3d;
mod view_2d;
mod view_2d_properties;
mod view_3d;
mod view_3d_properties;
mod visualizers;

use re_space_view::latest_at_with_overrides;
use re_types::archetypes::Pinhole;
use re_types::{Archetype as _, Loggable};
use re_viewer_context::ViewerContext;
use re_viewer_context::{ComponentFallbackProvider, ViewerContext};
pub use view_2d::SpatialSpaceView2D;
pub use view_3d::SpatialSpaceView3D;

// ---

use re_renderer::RenderContext;
use re_types::blueprint::components::BackgroundKind;
use re_types::components::{
    Color, ImagePlaneDistance, PinholeProjection, Resolution, TensorData, ViewCoordinates,
};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}

fn resolution_from_tensor(
    entity_db: &re_entity_db::EntityDb,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<Resolution> {
    // TODO(#5607): what should happen if the promise is still pending?
    entity_db
        .latest_at_component::<TensorData>(entity_path, query)
        .and_then(|tensor| {
            tensor
                .image_height_width_channels()
                .map(|hwc| Resolution([hwc[1] as f32, hwc[0] as f32].into()))
        })
}

/// Utility for querying a pinhole archetype instance.
fn query_pinhole(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    fallback_provider: &dyn ComponentFallbackProvider,
    view_state: &dyn re_viewer_context::SpaceViewState,
    data_result: &re_viewer_context::DataResult,
) -> Option<re_types::archetypes::Pinhole> {
    let resolver = ctx.recording().resolver();

    // TODO(jleibs): I hate everything about this
    let results = latest_at_with_overrides(
        ctx,
        None,
        query,
        data_result,
        re_types::archetypes::Pinhole::all_components()
            .iter()
            .copied(),
    );

    let image_from_camera = *results
        .get(PinholeProjection::name())?
        .to_dense(resolver)
        .flatten()
        .ok()?
        .first()?;

    let resolution = results
        .get_or_empty(Resolution::name())
        .to_dense(resolver)
        .flatten()
        .ok()
        .and_then(|r| r.first().copied())
        .or_else(|| resolution_from_tensor(ctx.recording(), query, &data_result.entity_path));

    let camera_xyz = results
        .get_or_empty(ViewCoordinates::name())
        .to_dense(resolver)
        .flatten()
        .ok()
        .and_then(|r| r.first().copied());

    let image_plane_distance = results
        .get_or_empty(ImagePlaneDistance::name())
        .to_dense(resolver)
        .flatten()
        .ok()
        .and_then(|r| r.first().copied())
        .or_else(|| {
            data_result.typed_fallback_for(
                ctx,
                fallback_provider,
                Some(Pinhole::name()),
                view_state,
            )
        });

    Some(re_types::archetypes::Pinhole {
        image_from_camera,
        resolution,
        camera_xyz,
        image_plane_distance,
    })
}

/// Deprecated utility for querying a pinhole archetype instance.
///
/// This function won't handle fallbacks correctly.
///
// TODO(andreas): This is duplicated into `re_viewport`
fn query_pinhole_legacy(
    entity_db: &re_entity_db::EntityDb,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    // TODO(#5607): what should happen if the promise is still pending?
    entity_db
        .latest_at_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(|image_from_camera| re_types::archetypes::Pinhole {
            image_from_camera: image_from_camera.value,
            resolution: entity_db
                .latest_at_component(entity_path, query)
                .map(|c| c.value)
                .or_else(|| resolution_from_tensor(entity_db, query, entity_path)),
            camera_xyz: entity_db
                .latest_at_component(entity_path, query)
                .map(|c| c.value),
            image_plane_distance: None,
        })
}

pub(crate) fn configure_background(
    ctx: &ViewerContext<'_>,
    background: &ViewProperty<'_>,
    render_ctx: &RenderContext,
    view_system: &dyn re_viewer_context::ComponentFallbackProvider,
    state: &dyn re_viewer_context::SpaceViewState,
) -> Result<(Option<re_renderer::QueueableDrawData>, re_renderer::Rgba), ViewPropertyQueryError> {
    use re_renderer::renderer;

    let kind: BackgroundKind = background.component_or_fallback(ctx, view_system, state)?;

    match kind {
        BackgroundKind::GradientDark => Ok((
            Some(
                renderer::GenericSkyboxDrawData::new(
                    render_ctx,
                    renderer::GenericSkyboxType::GradientDark,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        )),

        BackgroundKind::GradientBright => Ok((
            Some(
                renderer::GenericSkyboxDrawData::new(
                    render_ctx,
                    renderer::GenericSkyboxType::GradientBright,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        )),

        BackgroundKind::SolidColor => {
            let color: Color = background.component_or_fallback(ctx, view_system, state)?;
            Ok((None, color.into()))
        }
    }
}
