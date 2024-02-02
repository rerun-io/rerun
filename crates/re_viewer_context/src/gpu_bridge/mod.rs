//! Bridge to `re_renderer`

mod colormap;
mod re_renderer_callback;
mod tensor_to_gpu;

pub use colormap::colormap_dropdown_button_ui;
pub use re_renderer_callback::new_renderer_callback;
pub use tensor_to_gpu::{
    class_id_tensor_to_gpu, color_tensor_to_gpu, depth_tensor_to_gpu, tensor_to_gpu,
    texture_height_width_channels,
};

use crate::TensorStats;

// ----------------------------------------------------------------------------

use re_renderer::{
    renderer::{ColormappedTexture, RectangleOptions},
    resource_managers::{
        GpuTexture2D, Texture2DCreationDesc, TextureCreationError, TextureManager2DError,
    },
    RenderContext, ViewBuilder,
};

// ----------------------------------------------------------------------------

/// Errors that can happen when supplying a tensor range to the GPU.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum RangeError {
    /// This is weird. Should only happen with JPEGs, and those should have been decoded already
    #[error("Missing a range.")]
    MissingRange,
}

/// Get a valid, finite range for the gpu to use.
pub fn tensor_data_range_heuristic(
    tensor_stats: &TensorStats,
    data_type: re_types::tensor_data::TensorDataType,
) -> Result<[f32; 2], RangeError> {
    let (min, max) = tensor_stats.finite_range.ok_or(RangeError::MissingRange)?;

    let min = min as f32;
    let max = max as f32;

    // Apply heuristic for ranges that are typically expected depending on the data type and the finite (!) range.
    // (we ignore NaN/Inf values heres, since they are usually there by accident!)
    if data_type.is_float() && 0.0 <= min && max <= 1.0 {
        // Float values that are all between 0 and 1, assume that this is the range.
        Ok([0.0, 1.0])
    } else if 0.0 <= min && max <= 255.0 {
        // If all values are between 0 and 255, assume this is the range.
        // (This is very common, independent of the data type)
        Ok([0.0, 255.0])
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        Ok([min - 1.0, max + 1.0])
    } else {
        // Use range as is if nothing matches.
        Ok([min, max])
    }
}

/// Return whether a tensor should be assumed to be encoded in sRGB color space ("gamma space", no EOTF applied).
pub fn tensor_decode_srgb_gamma_heuristic(
    tensor_stats: &TensorStats,
    data_type: re_types::tensor_data::TensorDataType,
    channels: u32,
) -> Result<bool, RangeError> {
    if matches!(channels, 1 | 3 | 4) {
        let (min, max) = tensor_stats.finite_range.ok_or(RangeError::MissingRange)?;
        #[allow(clippy::if_same_then_else)]
        if 0.0 <= min && max <= 255.0 {
            // If the range is suspiciously reminding us of a "regular image", assume sRGB.
            Ok(true)
        } else if data_type.is_float() && 0.0 <= min && max <= 1.0 {
            // Floating point images between 0 and 1 are often sRGB as well.
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

// ----------------------------------------------------------------------------

pub fn viewport_resolution_in_pixels(clip_rect: egui::Rect, pixels_from_point: f32) -> [u32; 2] {
    let min = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let max = (clip_rect.max.to_vec2() * pixels_from_point).round();
    let resolution = max - min;
    [resolution.x as u32, resolution.y as u32]
}

pub fn try_get_or_create_texture<'a, Err: std::fmt::Display>(
    render_ctx: &RenderContext,
    texture_key: u64,
    try_create_texture_desc: impl FnOnce() -> Result<Texture2DCreationDesc<'a>, Err>,
) -> Result<GpuTexture2D, TextureManager2DError<Err>> {
    render_ctx.texture_manager_2d.get_or_try_create_with(
        texture_key,
        &render_ctx.gpu_resources.textures,
        try_create_texture_desc,
    )
}

pub fn get_or_create_texture<'a>(
    render_ctx: &RenderContext,
    texture_key: u64,
    create_texture_desc: impl FnOnce() -> Texture2DCreationDesc<'a>,
) -> Result<GpuTexture2D, TextureCreationError> {
    render_ctx.texture_manager_2d.get_or_create_with(
        texture_key,
        &render_ctx.gpu_resources.textures,
        create_texture_desc,
    )
}

/// Render the given image, respecting the clip rectangle of the given painter.
pub fn render_image(
    render_ctx: &re_renderer::RenderContext,
    egui_painter: &egui::Painter,
    image_rect_on_screen: egui::Rect,
    colormapped_texture: ColormappedTexture,
    texture_options: egui::TextureOptions,
    debug_name: &str,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    use re_renderer::renderer::{TextureFilterMag, TextureFilterMin};

    let viewport = egui_painter.clip_rect().intersect(image_rect_on_screen);
    if !viewport.is_positive() {
        return Ok(());
    }

    // Where in "world space" to paint the image.
    // This is an arbitrary selection.
    let space_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, image_rect_on_screen.size());

    let textured_rectangle = re_renderer::renderer::TexturedRect {
        top_left_corner_position: glam::vec3(space_rect.min.x, space_rect.min.y, 0.0),
        extent_u: glam::Vec3::X * space_rect.width(),
        extent_v: glam::Vec3::Y * space_rect.height(),
        colormapped_texture,
        options: RectangleOptions {
            texture_filter_magnification: match texture_options.magnification {
                egui::TextureFilter::Nearest => TextureFilterMag::Nearest,
                egui::TextureFilter::Linear => TextureFilterMag::Linear,
            },
            texture_filter_minification: match texture_options.minification {
                egui::TextureFilter::Nearest => TextureFilterMin::Nearest,
                egui::TextureFilter::Linear => TextureFilterMin::Linear,
            },
            multiplicative_tint: egui::Rgba::WHITE,
            ..Default::default()
        },
    };

    // ------------------------------------------------------------------------

    let pixels_from_points = egui_painter.ctx().pixels_per_point();
    let ui_from_space = egui::emath::RectTransform::from_to(space_rect, image_rect_on_screen);
    let space_from_ui = ui_from_space.inverse();
    let space_from_points = space_from_ui.scale().y;
    let points_from_pixels = 1.0 / egui_painter.ctx().pixels_per_point();
    let space_from_pixel = space_from_points * points_from_pixels;

    let resolution_in_pixel = viewport_resolution_in_pixels(viewport, pixels_from_points);
    anyhow::ensure!(resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0);

    let camera_position_space = space_from_ui.transform_pos(viewport.min);

    let top_left_position = glam::vec2(camera_position_space.x, camera_position_space.y);

    let target_config = re_renderer::view_builder::TargetConfiguration {
        name: debug_name.into(),
        resolution_in_pixel,
        view_from_world: macaw::IsoTransform::from_translation(-top_left_position.extend(0.0)),
        projection_from_view: re_renderer::view_builder::Projection::Orthographic {
            camera_mode: re_renderer::view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
            vertical_world_size: space_from_pixel * resolution_in_pixel[1] as f32,
            far_plane_distance: 1000.0,
        },
        viewport_transformation: re_renderer::RectTransform::IDENTITY,
        pixels_from_point: pixels_from_points,
        auto_size_config: Default::default(),
        outline_config: None,
    };

    let mut view_builder = ViewBuilder::new(render_ctx, target_config);

    view_builder.queue_draw(re_renderer::renderer::RectangleDrawData::new(
        render_ctx,
        &[textured_rectangle],
    )?);

    egui_painter.add(new_renderer_callback(
        view_builder,
        viewport,
        re_renderer::Rgba::TRANSPARENT,
    ));

    Ok(())
}
