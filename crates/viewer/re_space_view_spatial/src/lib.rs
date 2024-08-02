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
mod pickable_image;
mod picking;
mod proc_mesh;
mod scene_bounding_boxes;
mod space_camera_3d;
mod spatial_topology;
mod transform_component_tracker;
mod ui;
mod ui_2d;
mod ui_3d;
mod view_2d;
mod view_2d_properties;
mod view_3d;
mod view_3d_properties;
mod visualizers;

pub use view_2d::SpatialSpaceView2D;
pub use view_3d::SpatialSpaceView3D;

pub(crate) use pickable_image::PickableImageRect;

// ---

use re_space_view::DataResultQuery2 as _;
use re_viewer_context::{ViewContext, ViewerContext};

use re_renderer::RenderContext;
use re_types::blueprint::components::BackgroundKind;
use re_types::components::{Color, Resolution, TensorData};
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
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<Resolution> {
    entity_db
        .latest_at_component::<TensorData>(entity_path, query)
        .and_then(|(_index, tensor)| {
            tensor
                .image_height_width_channels()
                .map(|hwc| Resolution([hwc[1] as f32, hwc[0] as f32].into()))
        })
}

/// Utility for querying a pinhole archetype instance.
fn query_pinhole(
    ctx: &ViewContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    data_result: &re_viewer_context::DataResult,
) -> Option<re_types::archetypes::Pinhole> {
    let results = data_result
        .latest_at_with_blueprint_resolved_data::<re_types::archetypes::Pinhole>(ctx, query);

    let image_from_camera = results.get_mono()?;

    let resolution = results
        .get_mono()
        .or_else(|| resolution_from_tensor(ctx.recording(), query, &data_result.entity_path));

    let camera_xyz = results.get_mono();

    let image_plane_distance = Some(results.get_mono_with_fallback());

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
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    entity_db
        .latest_at_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(
            |(_index, image_from_camera)| re_types::archetypes::Pinhole {
                image_from_camera,
                resolution: entity_db
                    .latest_at_component(entity_path, query)
                    .map(|(_index, c)| c)
                    .or_else(|| resolution_from_tensor(entity_db, query, entity_path)),
                camera_xyz: entity_db
                    .latest_at_component(entity_path, query)
                    .map(|(_index, c)| c),
                image_plane_distance: None,
            },
        )
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
