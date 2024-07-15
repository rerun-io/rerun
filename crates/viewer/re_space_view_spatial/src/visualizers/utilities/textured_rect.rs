use re_chunk_store::RowId;
use re_log_types::EntityPath;
use re_renderer::renderer;
use re_types::{
    components::Colormap,
    tensor_data::{DecodedTensor, TensorDataMeaning},
};
use re_viewer_context::{gpu_bridge, TensorStatsCache, ViewerContext};

use crate::contexts::SpatialSceneEntityContext;

#[allow(clippy::too_many_arguments)]
pub fn tensor_to_textured_rect(
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneEntityContext<'_>,
    tensor_data_row_id: RowId,
    tensor: &DecodedTensor,
    meaning: TensorDataMeaning,
    multiplicative_tint: egui::Rgba,
    colormap: Option<Colormap>,
) -> Option<renderer::TexturedRect> {
    let Some(render_ctx) = ctx.render_ctx else {
        return None;
    };

    let [height, width, _] = tensor.image_height_width_channels()?;

    let debug_name = ent_path.to_string();
    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));

    match gpu_bridge::tensor_to_gpu(
        render_ctx,
        &debug_name,
        tensor_data_row_id,
        tensor,
        meaning,
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

            Some(renderer::TexturedRect {
                top_left_corner_position: ent_context
                    .world_from_entity
                    .transform_point3(glam::Vec3::ZERO),
                extent_u: ent_context
                    .world_from_entity
                    .transform_vector3(glam::Vec3::X * width as f32),
                extent_v: ent_context
                    .world_from_entity
                    .transform_vector3(glam::Vec3::Y * height as f32),
                colormapped_texture,
                options: renderer::RectangleOptions {
                    texture_filter_magnification,
                    texture_filter_minification,
                    multiplicative_tint,
                    depth_offset: ent_context.depth_offset,
                    outline_mask: ent_context.highlight.overall,
                },
            })
        }
        Err(err) => {
            re_log::error_once!("Failed to create texture for {debug_name:?}: {err}");
            None
        }
    }
}

pub fn bounding_box_for_textured_rect(
    textured_rect: &renderer::TexturedRect,
) -> re_math::BoundingBox {
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
