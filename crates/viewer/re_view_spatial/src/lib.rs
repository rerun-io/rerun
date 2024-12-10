//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod contexts;
mod eye;
mod heuristics;
mod max_image_dimension_subscriber;
mod mesh_cache;
mod mesh_loader;
mod pickable_textured_rect;
mod picking;
mod picking_ui;
mod picking_ui_pixel;
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

pub(crate) use pickable_textured_rect::{PickableRectSourceData, PickableTexturedRect};

// ---

use re_view::DataResultQuery as _;
use re_viewer_context::{ImageDecodeCache, ViewContext, ViewerContext};

use re_renderer::RenderContext;
use re_types::{
    archetypes,
    blueprint::components::BackgroundKind,
    components::{self, Color, ImageFormat, MediaType, Resolution},
    static_assert_struct_has_fields,
};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}

fn resolution_of_image_at(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<Resolution> {
    // First check assumptions:
    static_assert_struct_has_fields!(archetypes::Image, format: components::ImageFormat);
    static_assert_struct_has_fields!(archetypes::EncodedImage, blob: components::Blob);

    let db = ctx.recording();

    if let Some((_, image_format)) = db.latest_at_component::<ImageFormat>(entity_path, query) {
        // Normal `Image` archetype
        return Some(Resolution::from([
            image_format.width as f32,
            image_format.height as f32,
        ]));
    } else if let Some(((_time, row_id), blob)) =
        db.latest_at_component::<re_types::components::Blob>(entity_path, query)
    {
        // `archetypes.EncodedImage`

        let media_type = db
            .latest_at_component::<MediaType>(entity_path, query)
            .map(|(_, c)| c);

        let image = ctx
            .cache
            .entry(|c: &mut ImageDecodeCache| c.entry(row_id, &blob, media_type.as_ref()));

        if let Ok(image) = image {
            return Some(Resolution::from([
                image.format.width as f32,
                image.format.height as f32,
            ]));
        }
    }

    None
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

    let resolution = results.get_mono().or_else(|| {
        // If the Pinhole has no resolution, use the resolution for the image logged at the same path.
        // See https://github.com/rerun-io/rerun/issues/3852
        resolution_of_image_at(ctx.viewer_ctx, query, &data_result.entity_path)
    });

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
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<re_types::archetypes::Pinhole> {
    let entity_db = ctx.recording();
    entity_db
        .latest_at_component::<re_types::components::PinholeProjection>(entity_path, query)
        .map(
            |(_index, image_from_camera)| re_types::archetypes::Pinhole {
                image_from_camera,
                resolution: entity_db
                    .latest_at_component(entity_path, query)
                    .map(|(_index, c)| c)
                    .or_else(|| resolution_of_image_at(ctx, query, entity_path)),
                camera_xyz: entity_db
                    .latest_at_component(entity_path, query)
                    .map(|(_index, c)| c),
                image_plane_distance: None,
            },
        )
}

pub(crate) fn configure_background(
    ctx: &ViewerContext<'_>,
    background: &ViewProperty,
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
