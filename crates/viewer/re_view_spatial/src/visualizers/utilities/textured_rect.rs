use glam::Vec3;
use re_log_types::EntityPath;
use re_renderer::renderer;
use re_sdk_types::ArchetypeName;
use re_sdk_types::components::MagnificationFilter;
use re_viewer_context::{ColormapWithRange, ImageInfo, ImageStatsCache, ViewerContext, gpu_bridge};

use crate::contexts::SpatialSceneVisualizerInstructionContext;

fn mag_filter(filter: MagnificationFilter) -> renderer::TextureFilterMag {
    match filter {
        MagnificationFilter::Nearest => renderer::TextureFilterMag::Nearest,
        MagnificationFilter::Linear => renderer::TextureFilterMag::Linear,
        MagnificationFilter::Bicubic => renderer::TextureFilterMag::Bicubic,
    }
}

#[expect(clippy::too_many_arguments)]
pub fn textured_rect_from_image(
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
    image: &ImageInfo,
    colormap: Option<&ColormapWithRange>,
    multiplicative_tint: egui::Rgba,
    magnification_filter: MagnificationFilter,
    archetype_name: ArchetypeName,
) -> anyhow::Result<renderer::TexturedRect> {
    re_tracing::profile_function!();

    let debug_name = ent_path.to_string();
    let image_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut ImageStatsCache| c.entry(image));

    gpu_bridge::image_to_gpu(
        ctx.render_ctx(),
        &debug_name,
        image,
        &image_stats,
        &ent_context.annotations,
        colormap,
    )
    .map(|colormapped_texture| {
        let texture_filter_magnification = mag_filter(magnification_filter);

        let texture_filter_minification = match magnification_filter {
            MagnificationFilter::Nearest => {
                // For colormapped images (depth, segmentation), nearest makes sense
                // because interpolating before the colormap produces artifacts.
                // For color images, linear is generally better for minification.
                if colormapped_texture.color_mapper.is_on() {
                    renderer::TextureFilterMin::Nearest
                } else {
                    renderer::TextureFilterMin::Linear
                }
            }
            MagnificationFilter::Linear | MagnificationFilter::Bicubic => {
                renderer::TextureFilterMin::Linear
            }
        };

        let world_from_entity = ent_context
            .transform_info
            .single_transform_required_for_entity(ent_path, archetype_name)
            .as_affine3a();

        renderer::TexturedRect {
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
        }
    })
}
