use super::chroma_subsampling_converter::{
    ChromaSubsamplingConversionTask, ChromaSubsamplingPixelFormat,
};
use crate::{
    renderer::DrawError,
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
    Bt601 = 0,

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
    Bt709 = 2,
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

    #[error("Gpu based conversion for {label:?} did not succeed: {err}")]
    GpuBasedConversionError { label: DebugLabel, err: DrawError },

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
            SourceImageDataFormat::Y_UV12(_) => {
                ChromaSubsamplingPixelFormat::Y_UV12.expected_data_buffer_size(*width, *height)
            }
            SourceImageDataFormat::YUYV16(_) => {
                ChromaSubsamplingPixelFormat::YUYV16.expected_data_buffer_size(*width, *height)
            }
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
    ctx: &RenderContext,
    image_data: ImageDataDesc<'_>,
) -> Result<GpuTexture, ImageDataToTextureError> {
    re_tracing::profile_function!();

    image_data.validate(&ctx.device.limits())?;

    let ImageDataDesc {
        label,
        data,
        format: source_format,
        width,
        height,
    } = image_data;

    // Determine size of the texture the image data is uploaded into.
    // Reminder: We can't use raw buffers because of WebGL compatibility.
    let (data_texture_width, data_texture_height) = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => (width, height),
        SourceImageDataFormat::Y_UV12(_) => {
            ChromaSubsamplingPixelFormat::Y_UV12.expected_data_width_height(width, height)
        }
        SourceImageDataFormat::YUYV16(_) => {
            ChromaSubsamplingPixelFormat::YUYV16.expected_data_width_height(width, height)
        }
    };
    let data_texture_format = match source_format {
        SourceImageDataFormat::WgpuCompatible(format) => format,
        SourceImageDataFormat::Y_UV12(_) => {
            ChromaSubsamplingPixelFormat::Y_UV12.expected_data_texture_format()
        }
        SourceImageDataFormat::YUYV16(_) => {
            ChromaSubsamplingPixelFormat::YUYV16.expected_data_texture_format()
        }
    };

    // Allocate gpu belt data and upload it.
    let data_texture_label = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => label.clone(),
        SourceImageDataFormat::Y_UV12(_) | SourceImageDataFormat::YUYV16(_) => {
            format!("{label}_source_data").into()
        }
    };
    let data_texture = ctx.gpu_resources.textures.alloc(
        &ctx.device,
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        },
    );
    copy_data_to_texture(ctx, &data_texture, data.as_ref())?;

    // Build a converter task, feeding in the raw data.
    let converter_task = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => {
            // No further conversion needed, we're done here!
            return Ok(data_texture);
        }
        SourceImageDataFormat::Y_UV12(color_space) | SourceImageDataFormat::YUYV16(color_space) => {
            let chroma_format = match source_format {
                SourceImageDataFormat::WgpuCompatible(_) => unreachable!(),
                SourceImageDataFormat::Y_UV12(_) => ChromaSubsamplingPixelFormat::Y_UV12,
                SourceImageDataFormat::YUYV16(_) => ChromaSubsamplingPixelFormat::YUYV16,
            };
            ChromaSubsamplingConversionTask::new(
                ctx,
                chroma_format,
                color_space,
                data_texture,
                label.clone(),
                width,
                height,
            )
        }
    };

    // Once there's different gpu based conversions, we should probably trait-ify this so we can keep the basic steps.
    // Note that we execute the task right away, but the way things are set up (by means of using the `Renderer` framework)
    // it would be fairly easy to schedule this differently!
    let output_texture = converter_task
        .convert_input_data_to_texture(ctx)
        .map_err(|err| ImageDataToTextureError::GpuBasedConversionError { label, err })?;

    Ok(output_texture)
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
