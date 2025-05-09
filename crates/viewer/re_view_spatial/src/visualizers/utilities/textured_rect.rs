use glam::Vec3;

use re_log_types::EntityPath;
use re_renderer::renderer;
use re_viewer_context::{
    ColormapWithRange, ImageInfo, ImageStatsCache, ViewClass as _, ViewerContext, gpu_bridge,
};

use crate::{SpatialView2D, contexts::SpatialSceneEntityContext};

use super::SpatialViewVisualizerData;

#[allow(clippy::too_many_arguments)]
pub fn textured_rect_from_image(
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneEntityContext<'_>,
    image: &ImageInfo,
    colormap: Option<&ColormapWithRange>,
    multiplicative_tint: egui::Rgba,
    visualizer_name: &'static str,
    visualizer_data: &mut SpatialViewVisualizerData,
) -> Option<renderer::TexturedRect> {
    let debug_name = ent_path.to_string();
    let tensor_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut ImageStatsCache| c.entry(image));

    match gpu_bridge::image_to_gpu(
        ctx.render_ctx(),
        &debug_name,
        image,
        &tensor_stats,
        &ent_context.annotations,
        colormap,
    ) {
        Ok(colormapped_texture) => {
            // TODO(emilk): let users pick texture filtering.
            // Always use nearest for magnification: let users see crisp individual pixels when they zoom
            let texture_filter_magnification = renderer::TextureFilterMag::Nearest;

            // For minimization: we want a smooth linear (ideally mipmapped) filter for color images.
            // Note that this filtering is done BEFORE applying the color map!
            // For labeled/annotated/class_Id images we want nearest, because interpolating classes makes no sense.
            // Interpolating depth images _can_ make sense, but can also produce weird artifacts when there are big jumps (0.1m -> 100m),
            // so it's usually safer to turn off.
            // The best heuristic is this: if there is a color map being applied, use nearest.
            // TODO(emilk): apply filtering _after_ the color map?
            let texture_filter_minification = if colormapped_texture.color_mapper.is_on() {
                renderer::TextureFilterMin::Nearest
            } else {
                renderer::TextureFilterMin::Linear
            };

            let world_from_entity = ent_context
                .transform_info
                .single_entity_transform_required(ent_path, visualizer_name);

            let textured_rect = renderer::TexturedRect {
                top_left_corner_position: world_from_entity.transform_point3(Vec3::ZERO),
                extent_u: world_from_entity.transform_vector3(Vec3::X * image.width() as f32),
                extent_v: world_from_entity.transform_vector3(Vec3::Y * image.height() as f32),

                colormapped_texture,

                options: renderer::RectangleOptions {
                    texture_filter_magnification,
                    texture_filter_minification,
                    multiplicative_tint,
                    depth_offset: ent_context.depth_offset,
                    outline_mask: ent_context.highlight.overall,
                },
            };

            // Only update the bounding box if this is a 2D view.
            // This is avoids a cyclic relationship where the image plane grows
            // the bounds which in turn influence the size of the image plane.
            // See: https://github.com/rerun-io/rerun/issues/3728
            if ent_context.view_class_identifier == SpatialView2D::identifier() {
                visualizer_data.add_bounding_box(
                    ent_path.hash(),
                    bounding_box_for_textured_rect(&textured_rect),
                    world_from_entity,
                );
            }

            Some(textured_rect)
        }

        Err(err) => {
            re_log::error_once!("Failed to create texture for {debug_name:?}: {err}");
            None
        }
    }
}

fn bounding_box_for_textured_rect(textured_rect: &renderer::TexturedRect) -> re_math::BoundingBox {
    let left_top = textured_rect.top_left_corner_position;
    let extent_u = textured_rect.extent_u;
    let extent_v = textured_rect.extent_v;

    re_math::BoundingBox::from_points(
        [
            left_top,
            left_top + extent_u,
            left_top + extent_v,
            left_top + extent_v + extent_u,
        ]
        .into_iter(),
    )
}
