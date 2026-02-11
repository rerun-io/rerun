use glam::Vec3;
use re_log_types::EntityPath;
use re_renderer::renderer;
use re_sdk_types::ArchetypeName;
use re_viewer_context::{ColormapWithRange, ImageInfo, ImageStatsCache, ViewerContext, gpu_bridge};

use crate::contexts::SpatialSceneVisualizerInstructionContext;

pub fn textured_rect_from_image(
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
    image: &ImageInfo,
    colormap: Option<&ColormapWithRange>,
    multiplicative_tint: egui::Rgba,
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
                force_draw_with_transparency: false,
                depth_offset: ent_context.depth_offset,
                outline_mask: ent_context.highlight.overall,
            },
        }
    })
}
