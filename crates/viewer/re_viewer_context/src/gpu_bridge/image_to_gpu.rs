//! Upload images to [`re_renderer`].

use std::borrow::Cow;

use anyhow::Context as _;
use egui::{util::hash, Rangef};
use wgpu::TextureFormat;

use re_renderer::{
    config::DeviceCaps,
    pad_rgb_to_rgba,
    renderer::{ColorMapper, ColormappedTexture, ShaderDecoding},
    resource_managers::{
        ImageDataDesc, SourceImageDataFormat, YuvMatrixCoefficients, YuvPixelLayout, YuvRange,
    },
    RenderContext,
};
use re_types::components::ClassId;
use re_types::datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};
use re_types::image::ImageKind;

use crate::{
    gpu_bridge::colormap::colormap_to_re_renderer, image_info::ColormapWithRange, Annotations,
    ImageInfo, ImageStats,
};

use super::get_or_create_texture;

// ----------------------------------------------------------------------------

/// Returns a texture key for the given image.
///
/// If the key changes, we upload a new texture.
fn generate_texture_key(image: &ImageInfo) -> u64 {
    // We need to inclde anything that, if changes, should result in a new texture being uploaded.
    let ImageInfo {
        buffer_row_id: blob_row_id,
        buffer: _, // we hash `blob_row_id` instead; much faster!

        format,
        kind,
    } = image;

    hash((blob_row_id, format, kind))
}

/// `colormap` is currently only used for depth images.
pub fn image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    image: &ImageInfo,
    image_stats: &ImageStats,
    annotations: &Annotations,
    colormap: Option<&ColormapWithRange>,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    let texture_key = generate_texture_key(image);

    match image.kind {
        ImageKind::Color => {
            color_image_to_gpu(render_ctx, debug_name, texture_key, image, image_stats)
        }
        ImageKind::Depth => depth_image_to_gpu(
            render_ctx,
            debug_name,
            texture_key,
            image,
            image_stats,
            colormap,
        ),
        ImageKind::Segmentation => segmentation_image_to_gpu(
            render_ctx,
            debug_name,
            texture_key,
            image,
            image_stats,
            annotations,
        ),
    }
}

fn color_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageInfo,
    image_stats: &ImageStats,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    let image_format = image.format;

    let texture_handle = get_or_create_texture(render_ctx, texture_key, || {
        texture_creation_desc_from_color_image(render_ctx.device_caps(), image, debug_name)
    })
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    let texture_format = texture_handle.format();

    let shader_decoding = required_shader_decode(render_ctx.device_caps(), &image_format);

    // TODO(emilk): let the user specify the color space.
    let decode_srgb = texture_format == TextureFormat::Rgba8Unorm
        || image_decode_srgb_gamma_heuristic(image_stats, image_format);

    // Special casing for normalized textures used above:
    let range = if matches!(
        texture_format,
        TextureFormat::R8Unorm | TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
    ) {
        emath::Rangef::new(0.0, 1.0)
    } else if texture_format == TextureFormat::R8Snorm {
        emath::Rangef::new(-1.0, 1.0)
    } else if let Some(shader_decoding) = shader_decoding {
        match shader_decoding {
            ShaderDecoding::Bgr => image_data_range_heuristic(image_stats, &image_format),
        }
    } else {
        image_data_range_heuristic(image_stats, &image_format)
    };

    let color_mapper = if let Some(shader_decoding) = shader_decoding {
        match shader_decoding {
            // We only have 1D color maps, therefore BGR formats can't have color maps.
            ShaderDecoding::Bgr => ColorMapper::OffRGB,
        }
    } else if texture_format.components() == 1 {
        // TODO(andreas): support colormap property
        if decode_srgb {
            // Leave grayscale images unmolested - don't apply a colormap to them.
            ColorMapper::OffGrayscale
        } else {
            // This is something like a uint16 image, or a float image
            // with a range outside of 0-255 (see image_decode_srgb_gamma_heuristic).
            // `tensor_data_range_heuristic` will make sure we map this to a 0-1
            // range, and then we apply a gray colormap to it.
            ColorMapper::Function(re_renderer::Colormap::Grayscale)
        }
    } else {
        ColorMapper::OffRGB
    };

    // Assume that the texture has a separate (non-pre-multiplied) alpha.
    // TODO(wumpf): There should be a way to specify whether a texture uses pre-multiplied alpha or not.
    let multiply_rgb_with_alpha = image_format.has_alpha();

    let gamma = 1.0;

    re_log::trace_once!(
        "color_tensor_to_gpu {debug_name:?}, range: {range:?}, decode_srgb: {decode_srgb:?}, multiply_rgb_with_alpha: {multiply_rgb_with_alpha:?}, gamma: {gamma:?}, color_mapper: {color_mapper:?}",
    );

    Ok(ColormappedTexture {
        texture: texture_handle,
        range: [range.min, range.max],
        decode_srgb,
        multiply_rgb_with_alpha,
        gamma,
        color_mapper,
        shader_decoding,
    })
}

/// Get a valid, finite range for the gpu to use.
// TODO(#4624): The range should be determined by a `DataRange` component. In absence this, heuristics apply.
pub fn image_data_range_heuristic(image_stats: &ImageStats, image_format: &ImageFormat) -> Rangef {
    let (min, max) = image_stats.finite_range;

    let min = min as f32;
    let max = max as f32;

    // Apply heuristic for ranges that are typically expected depending on the data type and the finite (!) range.
    // (we ignore NaN/Inf values heres, since they are usually there by accident!)
    if image_format.is_float() && 0.0 <= min && max <= 1.0 {
        // Float values that are all between 0 and 1, assume that this is the range.
        Rangef::new(0.0, 1.0)
    } else if 0.0 <= min && max <= 255.0 {
        // If all values are between 0 and 255, assume this is the range.
        // (This is very common, independent of the data type)
        Rangef::new(0.0, 255.0)
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        Rangef::new(min - 1.0, max + 1.0)
    } else {
        // Use range as is if nothing matches.
        Rangef::new(min, max)
    }
}

/// Return whether an image should be assumed to be encoded in sRGB color space ("gamma space", no EOTF applied).
fn image_decode_srgb_gamma_heuristic(image_stats: &ImageStats, image_format: ImageFormat) -> bool {
    if image_format.pixel_format.is_some() {
        // Have to do the conversion because we don't use an `Srgb` texture format.
        true
    } else {
        let (min, max) = image_stats.finite_range;

        #[allow(clippy::if_same_then_else)]
        if 0.0 <= min && max <= 255.0 {
            // If the range is suspiciously reminding us of a "regular image", assume sRGB.
            true
        } else if image_format.datatype().is_float() && 0.0 <= min && max <= 1.0 {
            // Floating point images between 0 and 1 are often sRGB as well.
            true
        } else {
            false
        }
    }
}

/// Determines if and how the shader needs to decode the image.
///
/// Assumes creation as done by [`texture_creation_desc_from_color_image`].
pub fn required_shader_decode(
    device_caps: &DeviceCaps,
    image_format: &ImageFormat,
) -> Option<ShaderDecoding> {
    let color_model = image_format.color_model();

    if image_format.pixel_format.is_none() && color_model == ColorModel::BGR
        || color_model == ColorModel::BGRA
    {
        // U8 can be converted to RGBA without the shader's help since there's a format for it.
        if image_format.datatype() == ChannelDatatype::U8
            && device_caps.tier.support_bgra_textures()
        {
            None
        } else {
            Some(ShaderDecoding::Bgr)
        }
    } else {
        None
    }
}

/// Creates a [`ImageDataDesc`] for creating a texture from an [`ImageInfo`].
///
/// The resulting texture has requirements as describe by [`required_shader_decode`].
///
/// TODO(andreas): The consumer needs to be aware of bgr conversions. Other conversions are already taken care of upon upload.
pub fn texture_creation_desc_from_color_image<'a>(
    device_caps: &DeviceCaps,
    image: &'a ImageInfo,
    debug_name: &'a str,
) -> ImageDataDesc<'a> {
    re_tracing::profile_function!();

    // TODO(#7608): All image data ingestion conversions should all be handled by re_renderer!

    let (data, format) = if let Some(pixel_format) = image.format.pixel_format {
        let data = cast_slice_to_cow(image.buffer.as_slice());
        let coefficients = match pixel_format.yuv_matrix_coefficients() {
            re_types::image::YuvMatrixCoefficients::Bt601 => YuvMatrixCoefficients::Bt601,
            re_types::image::YuvMatrixCoefficients::Bt709 => YuvMatrixCoefficients::Bt709,
        };

        let range = match pixel_format.is_limited_yuv_range() {
            true => YuvRange::Limited,
            false => YuvRange::Full,
        };

        let format = match pixel_format {
            // For historical reasons, using Bt.709 for fully planar formats and Bt.601 for others.
            //
            // TODO(andreas): Investigate if there's underlying expectation for some of these (for instance I suspect that NV12 is "usually" BT601).
            // TODO(andreas): Expose coefficients. It's probably still the better default (for instance that's what jpeg still uses),
            // but should confirm & back that up!
            //
            PixelFormat::Y_U_V24_FullRange | PixelFormat::Y_U_V24_LimitedRange => {
                SourceImageDataFormat::Yuv {
                    layout: YuvPixelLayout::Y_U_V444,
                    range,
                    coefficients,
                }
            }

            PixelFormat::Y_U_V16_FullRange | PixelFormat::Y_U_V16_LimitedRange => {
                SourceImageDataFormat::Yuv {
                    layout: YuvPixelLayout::Y_U_V422,
                    range,
                    coefficients,
                }
            }

            PixelFormat::Y_U_V12_FullRange | PixelFormat::Y_U_V12_LimitedRange => {
                SourceImageDataFormat::Yuv {
                    layout: YuvPixelLayout::Y_U_V420,
                    range,
                    coefficients,
                }
            }

            PixelFormat::Y8_FullRange | PixelFormat::Y8_LimitedRange => {
                SourceImageDataFormat::Yuv {
                    layout: YuvPixelLayout::Y400,
                    range,
                    coefficients,
                }
            }

            PixelFormat::NV12 => SourceImageDataFormat::Yuv {
                layout: YuvPixelLayout::Y_UV420,
                range,
                coefficients,
            },

            PixelFormat::YUY2 => SourceImageDataFormat::Yuv {
                layout: YuvPixelLayout::YUYV422,
                range,
                coefficients,
            },
        };

        (data, format)
    } else {
        let color_model = image.format.color_model();
        let datatype = image.format.datatype();

        match (color_model, datatype) {
            // sRGB(A) handling is done by `ColormappedTexture`.
            // Why not use `Rgba8UnormSrgb`? Because premul must happen _before_ sRGB decode, so we can't
            // use a "Srgb-aware" texture like `Rgba8UnormSrgb` for RGBA.
            (ColorModel::RGB, ChannelDatatype::U8) => (
                pad_rgb_to_rgba(&image.buffer, u8::MAX).into(),
                SourceImageDataFormat::WgpuCompatible(TextureFormat::Rgba8Unorm),
            ),
            (ColorModel::RGBA, ChannelDatatype::U8) => (
                cast_slice_to_cow(&image.buffer),
                SourceImageDataFormat::WgpuCompatible(TextureFormat::Rgba8Unorm),
            ),

            // Make use of wgpu's BGR(A)8 formats if possible.
            //
            // From the pov of our on-the-fly decoding textured rect shader this is just a strange special case
            // given that it already has to deal with other BGR(A) formats.
            //
            // However, we have other places where we don't have the luxury of having a shader that can do the decoding for us.
            // In those cases we'd like to support as many formats as possible without decoding.
            //
            // (in some hopefully not too far future, re_renderer will have an internal conversion pipeline
            // that injects on-the-fly texture conversion from source formats before the consumer of a given texture is run
            // and caches the result alongside with the source data)
            //
            // See also [`required_shader_decode`] which lists this case as a format that does not need to be decoded.
            (ColorModel::BGR, ChannelDatatype::U8) => {
                let padded_data = pad_rgb_to_rgba(&image.buffer, u8::MAX).into();
                let texture_format = if required_shader_decode(device_caps, &image.format).is_some()
                {
                    TextureFormat::Rgba8Unorm
                } else {
                    TextureFormat::Bgra8Unorm
                };
                (
                    padded_data,
                    SourceImageDataFormat::WgpuCompatible(texture_format),
                )
            }
            (ColorModel::BGRA, ChannelDatatype::U8) => {
                let texture_format = if required_shader_decode(device_caps, &image.format).is_some()
                {
                    TextureFormat::Rgba8Unorm
                } else {
                    TextureFormat::Bgra8Unorm
                };
                (
                    cast_slice_to_cow(&image.buffer),
                    SourceImageDataFormat::WgpuCompatible(texture_format),
                )
            }

            _ => {
                // Fallback to general case:
                return general_texture_creation_desc_from_image(
                    debug_name,
                    image,
                    color_model,
                    datatype,
                );
            }
        }
    };

    ImageDataDesc {
        label: debug_name.into(),
        data,
        format,
        width_height: image.width_height(),
    }
}

fn depth_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageInfo,
    image_stats: &ImageStats,
    colormap_with_range: Option<&ColormapWithRange>,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    if let Some(pixel_format) = image.format.pixel_format {
        anyhow::bail!("Depth image does not support the PixelFormat {pixel_format}");
    }

    if image.format.color_model() != ColorModel::L {
        anyhow::bail!(
            "Depth image does not support the ColorModel {}",
            image.format.color_model()
        );
    }

    let datatype = image.format.datatype();

    let ColormapWithRange {
        value_range,
        colormap,
    } = colormap_with_range
        .cloned()
        .unwrap_or_else(|| ColormapWithRange::default_for_depth_images(image_stats));

    let texture = get_or_create_texture(render_ctx, texture_key, || {
        general_texture_creation_desc_from_image(debug_name, image, ColorModel::L, datatype)
    })
    .map_err(|err| anyhow::anyhow!("Failed to create depth texture: {err}"))?;

    Ok(ColormappedTexture {
        texture,
        range: value_range,
        decode_srgb: false,
        multiply_rgb_with_alpha: false,
        gamma: 1.0,
        color_mapper: ColorMapper::Function(colormap_to_re_renderer(colormap)),
        shader_decoding: None,
    })
}

fn segmentation_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageInfo,
    image_stats: &ImageStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    if let Some(pixel_format) = image.format.pixel_format {
        anyhow::bail!("Segmentation image does not support the PixelFormat {pixel_format}");
    }

    if image.format.color_model() != ColorModel::L {
        anyhow::bail!(
            "Segmentation image does not support the ColorModel {}",
            image.format.color_model()
        );
    }

    let datatype = image.format.datatype();

    let colormap_key = hash(annotations.row_id());

    let (_, mut max) = image_stats
        .range
        .ok_or_else(|| anyhow::anyhow!("compressed_tensor!?"))?;

    // We only support u8 and u16 class ids.
    // Any values greater than this will be unmapped in the segmentation image.
    max = max.min(65535.0);

    // We pack the colormap into a 2D texture so we don't go over the max texture size.
    // We only support u8 and u16 class ids, so 256^2 is the biggest texture we need.
    let num_colors = (max + 1.0) as usize;
    let colormap_width = 256;
    let colormap_height = num_colors.div_ceil(colormap_width);

    let colormap_texture_handle = get_or_create_texture(render_ctx, colormap_key, || {
        let data: Vec<u8> = (0..(colormap_width * colormap_height))
            .flat_map(|id| {
                let color = annotations
                    .resolved_class_description(Some(ClassId::from(id as u16)))
                    .annotation_info()
                    .color()
                    .unwrap_or(re_renderer::Color32::TRANSPARENT);
                color.to_array() // premultiplied!
            })
            .collect();

        ImageDataDesc {
            label: "class_id_colormap".into(),
            data: data.into(),
            format: SourceImageDataFormat::WgpuCompatible(TextureFormat::Rgba8UnormSrgb),
            width_height: [colormap_width as u32, colormap_height as u32],
        }
    })
    .context("Failed to create class_id_colormap.")?;

    let main_texture_handle = get_or_create_texture(render_ctx, texture_key, || {
        general_texture_creation_desc_from_image(debug_name, image, ColorModel::L, datatype)
    })
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(ColormappedTexture {
        texture: main_texture_handle,
        range: [0.0, (colormap_width * colormap_height) as f32],
        decode_srgb: false, // Setting this to true would affect the class ids, not the color they resolve to.
        multiply_rgb_with_alpha: false, // already premultiplied!
        gamma: 1.0,
        color_mapper: ColorMapper::Texture(colormap_texture_handle),
        shader_decoding: None,
    })
}

/// Uploads the image to a texture in a format that closely resembled the input.
/// Uses no `Unorm/Snorm` formats.
fn general_texture_creation_desc_from_image<'a>(
    debug_name: &str,
    image: &'a ImageInfo,
    color_model: ColorModel,
    datatype: ChannelDatatype,
) -> ImageDataDesc<'a> {
    re_tracing::profile_function!();

    let buf: &[u8] = image.buffer.as_ref();

    let (data, format) = match color_model {
        ColorModel::L => {
            match datatype {
                ChannelDatatype::U8 => (Cow::Borrowed(buf), TextureFormat::R8Uint),
                ChannelDatatype::U16 => (Cow::Borrowed(buf), TextureFormat::R16Uint),
                ChannelDatatype::U32 => (Cow::Borrowed(buf), TextureFormat::R32Uint),
                ChannelDatatype::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ChannelDatatype::I8 => (Cow::Borrowed(buf), TextureFormat::R8Sint),
                ChannelDatatype::I16 => (Cow::Borrowed(buf), TextureFormat::R16Sint),
                ChannelDatatype::I32 => (Cow::Borrowed(buf), TextureFormat::R32Sint),
                ChannelDatatype::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ChannelDatatype::F16 => (Cow::Borrowed(buf), TextureFormat::R16Float),
                ChannelDatatype::F32 => (Cow::Borrowed(buf), TextureFormat::R32Float),
                ChannelDatatype::F64 => (
                    // wgpu doesn't support f64 textures
                    narrow_f64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),
            }
        }

        // BGR->RGB conversion is done in the shader.
        ColorModel::RGB | ColorModel::BGR => {
            // There are no 3-channel textures in wgpu, so we need to pad to 4 channels.
            // What should we pad with? It depends on whether or not the shader interprets these as alpha.
            // To be safe, we pad with the MAX value of integers, and with 1.0 for floats.
            // TODO(emilk): tell the shader to ignore the alpha channel instead!

            match datatype {
                ChannelDatatype::U8 => (
                    pad_rgb_to_rgba(buf, u8::MAX).into(),
                    TextureFormat::Rgba8Uint,
                ),
                ChannelDatatype::U16 => (pad_cast_img(image, u16::MAX), TextureFormat::Rgba16Uint),
                ChannelDatatype::U32 => (pad_cast_img(image, u32::MAX), TextureFormat::Rgba32Uint),
                ChannelDatatype::U64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: u64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::I8 => (pad_cast_img(image, i8::MAX), TextureFormat::Rgba8Sint),
                ChannelDatatype::I16 => (pad_cast_img(image, i16::MAX), TextureFormat::Rgba16Sint),
                ChannelDatatype::I32 => (pad_cast_img(image, i32::MAX), TextureFormat::Rgba32Sint),
                ChannelDatatype::I64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: i64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::F16 => (
                    pad_cast_img(image, half::f16::from_f32(1.0)),
                    TextureFormat::Rgba16Float,
                ),
                ChannelDatatype::F32 => (pad_cast_img(image, 1.0_f32), TextureFormat::Rgba32Float),
                ChannelDatatype::F64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: f64| x as f32),
                    TextureFormat::Rgba32Float,
                ),
            }
        }

        // BGR->RGB conversion is done in the shader.
        ColorModel::RGBA | ColorModel::BGRA => {
            // TODO(emilk): premultiply alpha, or tell the shader to assume unmultiplied alpha

            match datatype {
                ChannelDatatype::U8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Uint),
                ChannelDatatype::U16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Uint),
                ChannelDatatype::U32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Uint),
                ChannelDatatype::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::I8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Sint),
                ChannelDatatype::I16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Sint),
                ChannelDatatype::I32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Sint),
                ChannelDatatype::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::F16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Float),
                ChannelDatatype::F32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Float),
                ChannelDatatype::F64 => (
                    // wgpu doesn't support f64 textures
                    narrow_f64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),
            }
        }
    };

    ImageDataDesc {
        label: debug_name.into(),
        data,
        format: SourceImageDataFormat::WgpuCompatible(format),
        width_height: image.width_height(),
    }
}

fn cast_slice_to_cow<From: bytemuck::Pod>(slice: &[From]) -> Cow<'_, [u8]> {
    bytemuck::cast_slice(slice).into()
}

// wgpu doesn't support u64 textures, so we need to narrow to f32:
fn narrow_u64_to_f32s(slice: &[u64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

// wgpu doesn't support i64 textures, so we need to narrow to f32:
fn narrow_i64_to_f32s(slice: &[i64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

// wgpu doesn't support f64 textures, so we need to narrow to f32:
fn narrow_f64_to_f32s(slice: &[f64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

/// Pad an RGB image to RGBA and cast the results to bytes.
fn pad_and_cast<T: Copy + bytemuck::Pod>(data: &[T], pad: T) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    // TODO(emilk): optimize by combining the two steps into one; avoiding one allocation and memcpy
    let padded: Vec<T> = pad_rgb_to_rgba(data, pad);
    let bytes: Vec<u8> = bytemuck::pod_collect_to_vec(&padded);
    bytes.into()
}

/// Pad an RGB image to RGBA and cast the results to bytes.
fn pad_cast_img<T: Copy + bytemuck::Pod>(img: &ImageInfo, pad: T) -> Cow<'static, [u8]> {
    pad_and_cast(&img.to_slice(), pad)
}

fn pad_and_narrow_and_cast<T: Copy + bytemuck::Pod>(
    data: &[T],
    pad: f32,
    narrow: impl Fn(T) -> f32,
) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();

    let floats: Vec<f32> = data
        .chunks_exact(3)
        .flat_map(|chunk| [narrow(chunk[0]), narrow(chunk[1]), narrow(chunk[2]), pad])
        .collect();
    bytemuck::pod_collect_to_vec(&floats).into()
}
