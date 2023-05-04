//! Bridge to `re_renderer`

mod tensor_to_gpu;
pub use tensor_to_gpu::tensor_to_gpu;

use crate::TensorStats;

// ----------------------------------------------------------------------------

use egui::mutex::Mutex;

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

    #[error("Non-finite range of values")]
    NonfiniteRange,
}

/// Get a valid, finite range for the gpu to use.
pub fn range(tensor_stats: &TensorStats) -> Result<[f32; 2], RangeError> {
    let (min, max) = tensor_stats.range.ok_or(RangeError::MissingRange)?;

    let min = min as f32;
    let max = max as f32;

    if !min.is_finite() || !max.is_finite() {
        Err(RangeError::NonfiniteRange)
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        Ok([min - 1.0, max + 1.0])
    } else {
        Ok([min, max])
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
    render_ctx: &mut RenderContext,
    texture_key: u64,
    try_create_texture_desc: impl FnOnce() -> Result<Texture2DCreationDesc<'a>, Err>,
) -> Result<GpuTexture2D, TextureManager2DError<Err>> {
    render_ctx.texture_manager_2d.get_or_try_create_with(
        texture_key,
        &mut render_ctx.gpu_resources.textures,
        try_create_texture_desc,
    )
}

pub fn get_or_create_texture<'a>(
    render_ctx: &mut RenderContext,
    texture_key: u64,
    create_texture_desc: impl FnOnce() -> Texture2DCreationDesc<'a>,
) -> Result<GpuTexture2D, TextureCreationError> {
    render_ctx.texture_manager_2d.get_or_create_with(
        texture_key,
        &mut render_ctx.gpu_resources.textures,
        create_texture_desc,
    )
}

/// Render a `re_render` view using the given clip rectangle.
pub fn renderer_paint_callback(
    render_ctx: &mut re_renderer::RenderContext,
    command_buffer: wgpu::CommandBuffer,
    view_builder: re_renderer::ViewBuilder,
    clip_rect: egui::Rect,
    pixels_from_point: f32,
) -> egui::PaintCallback {
    crate::profile_function!();

    slotmap::new_key_type! { pub struct ViewBuilderHandle; }

    type ViewBuilderMap = slotmap::SlotMap<ViewBuilderHandle, ViewBuilder>;

    // egui paint callback are copyable / not a FnOnce (this in turn is because egui primitives can be callbacks and are copyable)
    let command_buffer = std::sync::Arc::new(Mutex::new(Some(command_buffer)));

    let composition_view_builder_map = render_ctx
        .active_frame
        .per_frame_data_helper
        .entry::<ViewBuilderMap>()
        .or_insert_with(Default::default);
    let view_builder_handle = composition_view_builder_map.insert(view_builder);

    let screen_position = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let screen_position = glam::vec2(screen_position.x, screen_position.y);

    egui::PaintCallback {
        rect: clip_rect,
        callback: std::sync::Arc::new(
            egui_wgpu::CallbackFn::new()
                .prepare(
                    move |_device, _queue, _encoder, _paint_callback_resources| {
                        let mut command_buffer = command_buffer.lock();
                        vec![std::mem::replace(&mut *command_buffer, None)
                            .expect("egui_wgpu prepare callback called more than once")]
                    },
                )
                .paint(move |_info, render_pass, paint_callback_resources| {
                    crate::profile_scope!("paint");
                    // TODO(andreas): This should work as well but doesn't work in the 3d view.
                    //                  Looks like a bug in egui, but unclear what's going on.
                    //let clip_rect = info.clip_rect_in_pixels();

                    let ctx = paint_callback_resources.get::<RenderContext>().unwrap();
                    ctx.active_frame
                        .per_frame_data_helper
                        .get::<ViewBuilderMap>()
                        .unwrap()[view_builder_handle]
                        .composite(ctx, render_pass, screen_position);
                }),
        ),
    }
}

/// Render the given image, respecting the clip rectangle of the given painter.
pub fn render_image(
    render_ctx: &mut re_renderer::RenderContext,
    painter: &egui::Painter,
    image_rect_on_screen: egui::Rect,
    colormapped_texture: ColormappedTexture,
    texture_options: egui::TextureOptions,
    debug_name: &str,
) -> anyhow::Result<()> {
    crate::profile_function!();

    use re_renderer::renderer::{TextureFilterMag, TextureFilterMin};

    let clip_rect = painter.clip_rect().intersect(image_rect_on_screen);
    if !clip_rect.is_positive() {
        return Ok(());
    }

    // Where in "world space" to paint the image.
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

    let pixels_from_points = painter.ctx().pixels_per_point();
    let ui_from_space = egui::emath::RectTransform::from_to(space_rect, image_rect_on_screen);
    let space_from_ui = ui_from_space.inverse();
    let space_from_points = space_from_ui.scale().y;
    let points_from_pixels = 1.0 / painter.ctx().pixels_per_point();
    let space_from_pixel = space_from_points * points_from_pixels;

    let resolution_in_pixel = viewport_resolution_in_pixels(clip_rect, pixels_from_points);
    anyhow::ensure!(resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0);

    let camera_position_space = space_from_ui.transform_pos(clip_rect.min);

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

    view_builder.queue_draw(&re_renderer::renderer::RectangleDrawData::new(
        render_ctx,
        &[textured_rectangle],
    )?);

    let command_buffer = view_builder.draw(render_ctx, re_renderer::Rgba::TRANSPARENT)?;

    painter.add(renderer_paint_callback(
        render_ctx,
        command_buffer,
        view_builder,
        clip_rect,
        painter.ctx().pixels_per_point(),
    ));

    Ok(())
}
