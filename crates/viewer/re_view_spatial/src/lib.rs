//! Rerun Spatial Views
//!
//! Views that show entities in a 2D or 3D spatial relationship.

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
mod pinhole;
mod proc_mesh;
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

mod transform_cache;

pub use view_2d::SpatialView2D;
pub use view_3d::SpatialView3D;

pub(crate) use pickable_textured_rect::{PickableRectSourceData, PickableTexturedRect};
pub(crate) use pinhole::Pinhole;

// ---

use re_viewer_context::{ImageDecodeCache, ViewerContext};

use re_log_types::debug_assert_archetype_has_components;
use re_log_types::hash::Hash64;
use re_renderer::RenderContext;
use re_types::{
    blueprint::components::BackgroundKind,
    components::{Color, ImageFormat, MediaType, Resolution},
};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialViewKind {
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
    debug_assert_archetype_has_components!(re_types::archetypes::Image, format: re_types::components::ImageFormat);
    debug_assert_archetype_has_components!(re_types::archetypes::EncodedImage, blob: re_types::components::Blob);

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

        let image = ctx.store_context.caches.entry(|c: &mut ImageDecodeCache| {
            c.entry(Hash64::hash(row_id), &blob, media_type.as_ref())
        });

        if let Ok(image) = image {
            return Some(Resolution::from([
                image.format.width as f32,
                image.format.height as f32,
            ]));
        }
    }

    None
}

pub(crate) fn configure_background(
    ctx: &ViewerContext<'_>,
    background: &ViewProperty,
    render_ctx: &RenderContext,
    view_system: &dyn re_viewer_context::ComponentFallbackProvider,
    state: &dyn re_viewer_context::ViewState,
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
