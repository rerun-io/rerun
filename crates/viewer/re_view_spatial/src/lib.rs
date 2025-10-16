//! Rerun Spatial Views
//!
//! Views that show entities in a 2D or 3D spatial relationship.

mod caches;
mod contexts;
mod eye;
mod heuristics;
mod max_image_dimension_subscriber;
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
mod view_3d;
mod visualizers;

pub use ui::SpatialViewState;
pub use view_2d::SpatialView2D;
pub use view_3d::SpatialView3D;

pub(crate) use pickable_textured_rect::{PickableRectSourceData, PickableTexturedRect};
pub(crate) use pinhole::Pinhole;

// TODO(#8265): Used in tests, shouldn't be needed if it's part of blueprint.
#[doc(hidden)]
pub use eye::{Eye, ViewEye};

// ---

use re_viewer_context::ViewContext;

use re_types::{
    blueprint::{archetypes::Background, components::BackgroundKind},
    components::Color,
};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialViewKind {
        TwoD,
        ThreeD,
    }
}

pub fn configure_background(
    ctx: &ViewContext<'_>,
    background: &ViewProperty,
) -> Result<(Option<re_renderer::QueueableDrawData>, re_renderer::Rgba), ViewPropertyQueryError> {
    use re_renderer::renderer;

    let kind: BackgroundKind =
        background.component_or_fallback(ctx, &Background::descriptor_kind())?;

    match kind {
        BackgroundKind::GradientDark => Ok((
            Some(
                renderer::GenericSkyboxDrawData::new(
                    ctx.render_ctx(),
                    renderer::GenericSkyboxType::GradientDark,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        )),

        BackgroundKind::GradientBright => Ok((
            Some(
                renderer::GenericSkyboxDrawData::new(
                    ctx.render_ctx(),
                    renderer::GenericSkyboxType::GradientBright,
                )
                .into(),
            ),
            re_renderer::Rgba::TRANSPARENT, // All zero is slightly faster to clear usually.
        )),

        BackgroundKind::SolidColor => {
            let color: Color =
                background.component_or_fallback(ctx, &Background::descriptor_color())?;
            Ok((None, color.into()))
        }
    }
}
