use crate::{
    wgpu_resources::{GpuTexture, TextureDesc},
    DebugLabel, RenderContext, Texture2DBufferInfo,
};

/// Type of color space a given image is in.
///
/// This applies both to YUV and RGB formats, but if not specified otherwise
/// we assume BT.709 primaries for all RGB(A) 8bits per channel content (details below on [`ColorSpace::Bt709`]).
/// Since with YUV content the color space is often less clear, we always explicitely
/// specify it.
///
/// Ffmpeg's documentation has a short & good overview of these relationships:
/// <https://trac.ffmpeg.org/wiki/colorspace#WhatiscolorspaceWhyshouldwecare/>
#[derive(Clone, Copy, Debug)]
pub enum ColorSpace {
    /// BT.601 (aka. SDTV, aka. Rec.601)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion/>
    Bt601,

    /// BT.709 (aka. HDTV, aka. Rec.709)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion/>
    ///
    /// These are the same primaries we usually assume and use for all our rendering
    /// since they are the same primaries used by sRGB.
    /// <https://en.wikipedia.org/wiki/Rec._709#Relationship_to_sRGB/>
    /// The OETF/EOTF function (<https://en.wikipedia.org/wiki/Transfer_functions_in_imaging/>) is different,
    /// but for all other purposes they are the same.
    /// (The only reason for us to convert to optical units ("linear" instead of "gamma") is for
    /// lighting & tonemapping where we typically start out with an sRGB image!)
    Bt709,
    //
    // Not yet supported. These vary a lot more from the other two!
    //
    // /// BT.2020 (aka. PQ, aka. Rec.2020)
    // ///
    // /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.2020_conversion/>
    // BT2020_ConstantLuminance,
    // BT2020_NonConstantLuminance,
}

/// Image data format that can be converted to a wgpu texture.
///
/// Names follow a similar convention as Facebook's Ocean library
/// See <https://facebookresearch.github.io/ocean/docs/images/pixel_formats_and_plane_layout//>
// TODO(andreas): Right now this combines both color space and pixel format. Consider separating them similar to how we do on user facing APIs.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
pub enum SourceImageDataFormat {
    /// The source format is already in a wgpu compatible format.
    ///
    /// ⚠️ Only because a format is listed in `wgpu::TextureFormat` doesn't mean we can use it on the currently active backend.
    /// TODO(andreas): This is a temporary measure until we cover what rerun covers.
    ///                 We'd really like incoming data to not reason with [`wgpu::TextureFormat`] since it's so hard to know
    ///                 what's appropriate & available for a given device.
    WgpuCompatible(wgpu::TextureFormat),

    // TODO(andreas): Add rgb (3 channels!) formats.

    // -----------------------------------------------------------
    // Chroma downsampled formats, using YCbCr format.
    // -----------------------------------------------------------
    /// `Y_UV12` (aka `NV12`) is a YUV 4:2:0 chroma downsampled format with 12 bits per pixel and 8 bits per channel.
    ///
    /// First comes entire image in Y in one plane,
    /// followed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc.
    Y_UV12(ColorSpace),

    /// `YUYV16` (aka `YUYV` or `YUV2`), is a YUV 4:2:2 chroma downsampled format with 16 bits per pixel and 8 bits per channel.
    ///
    /// The order of the channels is Y0, U0, Y1, V0, all in the same plane.
    YUYV16(ColorSpace),
}

impl From<wgpu::TextureFormat> for SourceImageDataFormat {
    fn from(format: wgpu::TextureFormat) -> Self {
        Self::WgpuCompatible(format)
    }
}

/// Error that can occur when converting image data to a texture.
#[derive(thiserror::Error, Debug)]
pub enum ImageDataToTextureError {
    #[error("Texture with debug label {0:?} has zero width or height!")]
    ZeroSize(DebugLabel),

    #[error("Texture was {width}x{height}, larger than the max of {max_texture_dimension_2d}")]
    TooLarge {
        width: u32,
        height: u32,
        max_texture_dimension_2d: u32,
    },

    #[error("Invalid data length for texture with debug label {label:?}. Expected {expected} bytes, got {actual} bytes")]
    InvalidDataLength {
        label: DebugLabel,
        expected: usize,
        actual: usize,
    },

    #[error(transparent)]
    CpuWriteGpuReadError(#[from] crate::allocator::CpuWriteGpuReadError),

    #[error(
        "Texture with debug label {label:?} has a format {format:?} that data can't be transferred to!"
    )]
    UnsupportedFormatForTransfer {
        label: DebugLabel,
        format: wgpu::TextureFormat,
    },

    // TODO(andreas): As we stop using `wgpu::TextureFormat` for input, this should become obsolete.
    #[error("Unsupported texture format {0:?}")]
    UnsupportedTextureFormat(wgpu::TextureFormat),
}

/// Describes image data for the purpose of creating a 2D texture.
///
/// Arbitrary (potentially gpu based) conversions may be performed to upload the data to the GPU.
pub struct ImageDataDesc<'a> {
    pub label: DebugLabel,

    /// Data for the highest mipmap level.
    ///
    /// Data is expected to be tightly packed.
    /// I.e. it is *not* padded according to wgpu buffer->texture transfer rules, padding will happen on the fly if necessary.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: std::borrow::Cow<'a, [u8]>,
    pub format: SourceImageDataFormat,
    pub width: u32,
    pub height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl<'a> ImageDataDesc<'a> {
    fn validate(&self, limits: &wgpu::Limits) -> Result<(), ImageDataToTextureError> {
        let Self {
            label,
            data,
            format,
            width,
            height,
        } = self;

        if *width == 0 || *height == 0 {
            return Err(ImageDataToTextureError::ZeroSize(label.clone()));
        }

        let max_texture_dimension_2d = limits.max_texture_dimension_2d;
        if *width > max_texture_dimension_2d || *height > max_texture_dimension_2d {
            return Err(ImageDataToTextureError::TooLarge {
                width: *width,
                height: *height,
                max_texture_dimension_2d,
            });
        }

        let num_pixels = *width as usize * *height as usize;
        let expected_num_bytes = match format {
            SourceImageDataFormat::WgpuCompatible(format) => {
                num_pixels
                    * format
                        .block_copy_size(None)
                        .ok_or(ImageDataToTextureError::UnsupportedTextureFormat(*format))?
                        as usize
            }
            SourceImageDataFormat::Y_UV12(_) => 12 * num_pixels / 8,
            SourceImageDataFormat::YUYV16(_) => 16 * num_pixels / 8,
        };

        // TODO(andreas): Nv12 needs height divisible by 2?
        if data.len() != expected_num_bytes {
            return Err(ImageDataToTextureError::InvalidDataLength {
                label: label.clone(),
                expected: expected_num_bytes,
                actual: data.len(),
            });
        }

        Ok(())
    }
}

/// Takes raw image data and transfers & converts it to a GPU texture.
///
/// Schedules render passes to convert the data to a samplable textures if needed.
///
/// Generally, we currently do *not* sRGB converting formats like [`wgpu::TextureFormat::Rgba8UnormSrgb`] in order to...
/// * have the same shader code path for high precision formats (e.g. an f16 texture that _still_ encodes sRGB data)
/// * handle alpha pre-multiply on the fly (needs to happen before sRGB decode to linear)
///
/// Implementation note:
/// Since we're targeting WebGL, all data has always to be uploaded into textures (we can't use raw buffers!).
/// Buffer->Texture copies have restrictions on row padding, so any approach where we first
/// allocate gpu readable memory and hand it to the user would make the API a lot more complicated.
pub fn transfer_image_data_to_texture(
    render_ctx: &RenderContext,
    image_data: ImageDataDesc<'_>,
) -> Result<GpuTexture, ImageDataToTextureError> {
    re_tracing::profile_function!();

    image_data.validate(&render_ctx.device.limits())?;

    let ImageDataDesc {
        label,
        data,
        format: source_format,
        width,
        height,
    } = image_data;

    // Determine size of the texture the image data is uploaded into.
    // Reminder: We can't use raw buffers because of WebGL compatibility.
    let (data_texture_width, data_texture_height, data_texture_format) = match source_format {
        SourceImageDataFormat::WgpuCompatible(format) => (width, height, format),

        SourceImageDataFormat::Y_UV12(_) => {
            (width, height + height / 2, wgpu::TextureFormat::R8Uint)
        }
        SourceImageDataFormat::YUYV16(_) => (width * 2, height, wgpu::TextureFormat::R8Uint),
    };

    // Determine whether this format needs a conversion step.
    // If false, the data_texture is already the final output.
    #[allow(clippy::match_same_arms)]
    let needs_conversion = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => false,
        SourceImageDataFormat::Y_UV12(_) => true,
        SourceImageDataFormat::YUYV16(_) => true,
    };

    // Allocate gpu belt data and upload it.
    let data_texture_label = if needs_conversion {
        format!("{label}_source_data").into()
    } else {
        label
    };
    let data_texture_usage = if needs_conversion {
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT
    } else {
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
    };
    let data_texture = render_ctx.gpu_resources.textures.alloc(
        &render_ctx.device,
        &TextureDesc {
            label: data_texture_label,
            size: wgpu::Extent3d {
                width: data_texture_width,
                height: data_texture_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, // We don't have mipmap level generation yet!
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: data_texture_format,
            usage: data_texture_usage,
        },
    );
    copy_data_to_texture(render_ctx, &data_texture, data.as_ref())?;

    if !needs_conversion {
        return Ok(data_texture);
    }

    // TODO: if needed, schedule render pass with fragment shader to convert data
    // -> this may need render pipelines & bind layouts.
    //    -> Just use `ctx.renderer` in order to encapsulate the necessary data for different conversion steps.
    //       Bit of a missuse but should work & scale (!) just fine.

    // TODO:
    Ok(data_texture)
}

fn copy_data_to_texture(
    render_ctx: &RenderContext,
    data_texture: &GpuTexture,
    data: &[u8],
) -> Result<(), ImageDataToTextureError> {
    re_tracing::profile_function!();

    let buffer_info =
        Texture2DBufferInfo::new(data_texture.texture.format(), data_texture.texture.size());

    let mut cpu_write_gpu_read_belt = render_ctx.cpu_write_gpu_read_belt.lock();
    let mut gpu_read_buffer = cpu_write_gpu_read_belt.allocate::<u8>(
        &render_ctx.device,
        &render_ctx.gpu_resources.buffers,
        buffer_info.buffer_size_padded as usize,
    )?;

    if buffer_info.buffer_size_padded as usize == data.len() {
        // Fast path: Just copy the data over as-is.
        gpu_read_buffer.extend_from_slice(data)?;
    } else {
        // Copy row by row in order to jump over padding bytes.
        let bytes_per_row_unpadded = buffer_info.bytes_per_row_unpadded as usize;
        let num_padding_bytes_per_row =
            buffer_info.bytes_per_row_padded as usize - bytes_per_row_unpadded;
        debug_assert!(
            num_padding_bytes_per_row > 0,
            "No padding bytes, but the unpadded buffer size is not equal to the unpadded buffer."
        );

        for row in 0..data_texture.texture.size().height as usize {
            gpu_read_buffer.extend_from_slice(
                &data[(row * bytes_per_row_unpadded)
                    ..(row * bytes_per_row_unpadded + bytes_per_row_unpadded)],
            )?;
            gpu_read_buffer.add_n(0, num_padding_bytes_per_row)?;
        }
    }

    let mut before_view_builder_encoder =
        render_ctx.active_frame.before_view_builder_encoder.lock();
    gpu_read_buffer
        .copy_to_texture2d_entire_first_layer(before_view_builder_encoder.get(), data_texture)?;

    Ok(())
}
