use re_log_types::{
    component_types::{Tensor, TensorCastError},
    TensorDataType,
};
use re_renderer::{
    renderer::{ColormappedTexture, RectangleDrawData, TextureFilterMag, TextureFilterMin},
    resource_managers::Texture2DCreationDesc,
    view_builder::{TargetConfiguration, ViewBuilder},
};

use crate::misc::caches::TensorStats;

use super::{
    ui::{selected_tensor_slice, SliceSelection},
    ViewTensorState,
};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TensorUploadError {
    #[error(transparent)]
    TensorCastError(#[from] TensorCastError),

    #[error("Expected a 2D slice")]
    Not2D,

    /// This is weird. Should only happen with JPEGs, and those should have been decoded already
    #[error("Missing a range.")]
    MissingRange,

    #[error("Non-finite range of vlaues")]
    NonfiniteRange,
}

pub fn colormapped_texture(
    render_ctx: &mut re_renderer::RenderContext,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
    state: &ViewTensorState,
) -> Result<ColormappedTexture, TensorUploadError> {
    let (min, max) = range(tensor_stats)?;
    let texture = upload_texture_slice_to_gpu(render_ctx, tensor, state.slice())?;

    let color_mapping = state.color_mapping();

    Ok(ColormappedTexture {
        texture,
        range: [min as f32, max as f32],
        gamma: color_mapping.gamma,
        color_mapper: Some(re_renderer::renderer::ColorMapper::Function(
            color_mapping.map,
        )),
    })
}

fn range(tensor_stats: &TensorStats) -> Result<(f64, f64), TensorUploadError> {
    let (min, max) = tensor_stats.range.ok_or(TensorUploadError::MissingRange)?;

    if !min.is_finite() || !max.is_finite() {
        Err(TensorUploadError::NonfiniteRange)
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        Ok((min - 1.0, max + 1.0))
    } else {
        Ok((min, max))
    }
}

fn upload_texture_slice_to_gpu(
    render_ctx: &mut re_renderer::RenderContext,
    tensor: &Tensor,
    slice_selection: &SliceSelection,
) -> Result<re_renderer::resource_managers::GpuTexture2DHandle, TensorUploadError> {
    let id = egui::util::hash((tensor.id(), slice_selection));

    crate::misc::tensor_to_gpu::get_or_create_texture(render_ctx, id, || {
        texture_desc_from_tensor(tensor, slice_selection)
    })
}

fn texture_desc_from_tensor(
    tensor: &Tensor,
    slice_selection: &SliceSelection,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    use wgpu::TextureFormat;
    match tensor.dtype() {
        TensorDataType::U8 => {
            let tensor = ndarray::ArrayViewD::<u8>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R8Uint, |x| x)
        }
        TensorDataType::U16 => {
            let tensor = ndarray::ArrayViewD::<u16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Uint, |x| x)
        }
        TensorDataType::U32 => {
            let tensor = ndarray::ArrayViewD::<u32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Uint, |x| x)
        }
        TensorDataType::U64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<u64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: u64| x as f32,
            )
        }
        TensorDataType::I8 => {
            let tensor = ndarray::ArrayViewD::<i8>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R8Sint, |x| x)
        }
        TensorDataType::I16 => {
            let tensor = ndarray::ArrayViewD::<i16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Sint, |x| x)
        }
        TensorDataType::I32 => {
            let tensor = ndarray::ArrayViewD::<i32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Sint, |x| x)
        }
        TensorDataType::I64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<i64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: i64| x as f32,
            )
        }
        TensorDataType::F16 => {
            let tensor = ndarray::ArrayViewD::<half::f16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Float, |x| x)
        }
        TensorDataType::F32 => {
            let tensor = ndarray::ArrayViewD::<f32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Float, |x| x)
        }
        TensorDataType::F64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<f64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: f64| x as f32,
            )
        }
    }
}

fn to_texture_desc<From: Copy, To: bytemuck::Pod>(
    tensor: &ndarray::ArrayViewD<'_, From>,
    slice_selection: &SliceSelection,
    format: wgpu::TextureFormat,
    caster: impl Fn(From) -> To,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    use ndarray::Dimension as _;

    let slice = selected_tensor_slice(slice_selection, tensor);
    let slice = slice
        .into_dimensionality::<ndarray::Ix2>()
        .map_err(|_err| TensorUploadError::Not2D)?;

    let (height, width) = slice.raw_dim().into_pattern();
    let mut pixels: Vec<To> = vec![To::zeroed(); height * width];
    let pixels_view = ndarray::ArrayViewMut2::from_shape(slice.raw_dim(), pixels.as_mut_slice())
        .expect("Mismatched length.");
    ndarray::Zip::from(pixels_view)
        .and(slice)
        .for_each(|pixel: &mut To, value: &From| {
            *pixel = caster(*value);
        });

    Ok(Texture2DCreationDesc {
        label: "tensor_slice".into(),
        data: bytemuck::pod_collect_to_vec(&pixels).into(),
        format,
        width: width as u32,
        height: height as u32,
    })
}

// ----------------------------------------------------------------------------

pub fn paint(
    render_ctx: &mut re_renderer::RenderContext,
    painter: &egui::Painter,
    slice_size: egui::Vec2,
    image_position_on_screen: egui::Rect,
    colormapped_texture: ColormappedTexture,
    texture_options: egui::TextureOptions,
) -> anyhow::Result<()> {
    crate::profile_function!();

    let space_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, slice_size);

    let textured_rectangle = re_renderer::renderer::TexturedRect {
        top_left_corner_position: glam::Vec3::ZERO,
        extent_u: glam::Vec3::X * slice_size.x,
        extent_v: glam::Vec3::Y * slice_size.y,
        colormapped_texture,
        texture_filter_magnification: match texture_options.magnification {
            egui::TextureFilter::Nearest => TextureFilterMag::Nearest,
            egui::TextureFilter::Linear => TextureFilterMag::Linear,
        },
        texture_filter_minification: match texture_options.minification {
            egui::TextureFilter::Nearest => TextureFilterMin::Nearest,
            egui::TextureFilter::Linear => TextureFilterMin::Linear,
        },
        multiplicative_tint: egui::Rgba::WHITE,
        depth_offset: 0,
        outline_mask: Default::default(),
    };

    // ------------------------------------------------------------------------

    let pixels_from_points = painter.ctx().pixels_per_point();
    let ui_from_space = egui::emath::RectTransform::from_to(space_rect, image_position_on_screen);
    let space_from_ui = ui_from_space.inverse();
    let space_from_points = space_from_ui.scale().y;
    let points_from_pixels = 1.0 / painter.ctx().pixels_per_point();
    let space_from_pixel = space_from_points * points_from_pixels;

    let resolution_in_pixel = get_viewport(painter.clip_rect(), pixels_from_points);
    anyhow::ensure!(resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0);

    let camera_position_space = space_from_ui.transform_pos(painter.clip_rect().min);

    let top_left_position = glam::vec2(camera_position_space.x, camera_position_space.y);
    let target_config = TargetConfiguration {
        name: "tensor_view".into(),
        resolution_in_pixel,
        view_from_world: macaw::IsoTransform::from_translation(-top_left_position.extend(0.0)),
        projection_from_view: re_renderer::view_builder::Projection::Orthographic {
            camera_mode: re_renderer::view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
            vertical_world_size: space_from_pixel * resolution_in_pixel[1] as f32,
            far_plane_distance: 1000.0,
        },
        pixels_from_point: pixels_from_points,
        auto_size_config: Default::default(),
        outline_config: None,
    };

    // TODO(andreas): separate setup for viewbuilder doesn't make sense.
    let mut view_builder = ViewBuilder::default();
    view_builder.setup_view(render_ctx, target_config)?;

    view_builder.queue_draw(&RectangleDrawData::new(render_ctx, &[textured_rectangle])?);

    let command_buffer = view_builder.draw(render_ctx, re_renderer::Rgba::TRANSPARENT)?;

    painter.add(
        crate::ui::view_spatial::ui_renderer_bridge::renderer_paint_callback(
            render_ctx,
            command_buffer,
            view_builder,
            painter.clip_rect(),
            painter.ctx().pixels_per_point(),
        ),
    );

    Ok(())
}

fn get_viewport(clip_rect: egui::Rect, pixels_from_point: f32) -> [u32; 2] {
    let min = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let max = (clip_rect.max.to_vec2() * pixels_from_point).round();
    let resolution = max - min;
    [resolution.x as u32, resolution.y as u32]
}
