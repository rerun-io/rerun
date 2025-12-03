//! For an overview of image data interpretation check `re_video`'s decoder docs!

use super::yuv_converter::{
    YuvFormatConversionTask, YuvMatrixCoefficients, YuvPixelLayout, YuvRange,
};
use crate::renderer::DrawError;
use crate::wgpu_resources::{GpuTexture, TextureDesc};
use crate::{DebugLabel, RenderContext, Texture2DBufferInfo};

/// Image data format that can be converted to a wgpu texture.
// TODO(andreas): Right now this combines both color space and pixel format. Consider separating them similar to how we do on user facing APIs.
#[derive(Clone, Copy, Debug)]
pub enum SourceImageDataFormat {
    /// The source format is already in a wgpu compatible format.
    ///
    /// ⚠️ Only because a format is listed in `wgpu::TextureFormat` doesn't mean we can use it on the currently active backend.
    /// TODO(andreas): This is a temporary measure until we cover what rerun covers.
    ///                 We'd really like incoming data to not reason with [`wgpu::TextureFormat`] since it's so hard to know
    ///                 what's appropriate & available for a given device.
    WgpuCompatible(wgpu::TextureFormat),

    /// YUV (== `YCbCr`) formats, typically using chroma downsampling.
    ///
    /// Does not handle chroma sample locations.
    Yuv {
        layout: YuvPixelLayout,
        coefficients: YuvMatrixCoefficients,
        range: YuvRange,
    },
    //
    // TODO(#10648): Add rgb (3 channels!) formats.
}

impl From<wgpu::TextureFormat> for SourceImageDataFormat {
    fn from(format: wgpu::TextureFormat) -> Self {
        Self::WgpuCompatible(format)
    }
}

/// Error that can occur when converting image data to a texture.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ImageDataToTextureError {
    #[error("Texture {0:?} has zero width or height!")]
    ZeroSize(DebugLabel),

    #[error(
        "Texture {label:?} was {width}x{height}, larger than the max of {max_texture_dimension_2d}"
    )]
    TooLarge {
        label: DebugLabel,
        width: u32,
        height: u32,
        max_texture_dimension_2d: u32,
    },

    #[error(
        "Invalid data length for texture {label:?}. Expected {expected} bytes, got {actual} bytes"
    )]
    InvalidDataLength {
        label: DebugLabel,
        expected: usize,
        actual: usize,
    },

    #[error(transparent)]
    CpuWriteGpuReadError(#[from] crate::allocator::CpuWriteGpuReadError),

    #[error("Texture {label:?} has a format {format:?} that data can't be transferred to!")]
    UnsupportedFormatForTransfer {
        label: DebugLabel,
        format: wgpu::TextureFormat,
    },

    #[error("Gpu-based conversion for texture {label:?} did not succeed: {err}")]
    GpuBasedConversionError { label: DebugLabel, err: DrawError },

    #[error(
        "Texture {label:?} has invalid texture usage flags: {actual_usage:?}, expected at least {required_usage:?}"
    )]
    InvalidTargetTextureUsageFlags {
        label: DebugLabel,
        actual_usage: wgpu::TextureUsages,
        required_usage: wgpu::TextureUsages,
    },

    #[error(
        "Texture {label:?} has invalid texture format: {actual_format:?}, expected at least {required_format:?}"
    )]
    InvalidTargetTextureFormat {
        label: DebugLabel,
        actual_format: wgpu::TextureFormat,
        required_format: wgpu::TextureFormat,
    },

    // TODO(andreas): As we stop using `wgpu::TextureFormat` for input, this should become obsolete.
    #[error("Unsupported texture format {0:?}")]
    UnsupportedTextureFormat(wgpu::TextureFormat),
}

/// Describes image data for the purpose of creating a 2D texture.
///
/// Arbitrary (potentially gpu based) conversions may be performed to upload the data to the GPU.
pub struct ImageDataDesc<'a> {
    /// If this desc is not used for a texture update, this label is used for the target texture.
    /// Otherwise, it may still used for any intermediate resources that may be required during the conversion process.
    pub label: DebugLabel,

    /// Data for the highest mipmap level.
    ///
    /// Data is expected to be tightly packed.
    /// I.e. it is *not* padded according to wgpu buffer->texture transfer rules, padding will happen on the fly if necessary.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: std::borrow::Cow<'a, [u8]>,
    pub format: SourceImageDataFormat,

    /// The size of the resulting output texture / the semantic size of the image data.
    ///
    /// The distinction is in particular important for planar formats.
    /// Which may be represented as a larger texture than the image they represent.
    /// With the output always being a ("mainstream" gpu readable) texture format, the output texture's
    /// width/height is the semantic width/height of the image data!
    pub width_height: [u32; 2],
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl ImageDataDesc<'_> {
    fn validate(
        &self,
        limits: &wgpu::Limits,
        target_texture_desc: &TextureDesc,
    ) -> Result<(), ImageDataToTextureError> {
        let Self {
            label,
            data,
            format,
            width_height,
        } = self;

        if !target_texture_desc
            .usage
            .contains(self.target_texture_usage_requirements())
        {
            return Err(ImageDataToTextureError::InvalidTargetTextureUsageFlags {
                label: target_texture_desc.label.clone(),
                actual_usage: target_texture_desc.usage,
                required_usage: self.target_texture_usage_requirements(),
            });
        }
        if target_texture_desc.format != self.target_texture_format() {
            return Err(ImageDataToTextureError::InvalidTargetTextureFormat {
                label: target_texture_desc.label.clone(),
                actual_format: target_texture_desc.format,
                required_format: self.target_texture_format(),
            });
        }

        if width_height[0] == 0 || width_height[1] == 0 {
            return Err(ImageDataToTextureError::ZeroSize(label.clone()));
        }

        let max_texture_dimension_2d = limits.max_texture_dimension_2d;
        if width_height[0] > max_texture_dimension_2d || width_height[1] > max_texture_dimension_2d
        {
            return Err(ImageDataToTextureError::TooLarge {
                label: label.clone(),
                width: width_height[0],
                height: width_height[1],
                max_texture_dimension_2d,
            });
        }

        let num_pixels = width_height[0] as usize * width_height[1] as usize;
        let expected_num_bytes = match format {
            SourceImageDataFormat::WgpuCompatible(format) => {
                num_pixels
                    * format
                        .block_copy_size(None)
                        .ok_or(ImageDataToTextureError::UnsupportedTextureFormat(*format))?
                        as usize
            }
            SourceImageDataFormat::Yuv { layout: format, .. } => {
                format.num_data_buffer_bytes(*width_height)
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

    /// The texture usages required in order to store this image data.
    pub fn target_texture_usage_requirements(&self) -> wgpu::TextureUsages {
        match self.format {
            SourceImageDataFormat::WgpuCompatible(_) => wgpu::TextureUsages::COPY_DST, // Data arrives via raw data copy.
            SourceImageDataFormat::Yuv { .. } => {
                YuvFormatConversionTask::REQUIRED_TARGET_TEXTURE_USAGE_FLAGS
            }
        }
    }

    /// The texture format required in order to store this image data.
    pub fn target_texture_format(&self) -> wgpu::TextureFormat {
        match self.format {
            SourceImageDataFormat::WgpuCompatible(format) => format,
            SourceImageDataFormat::Yuv { .. } => YuvFormatConversionTask::OUTPUT_FORMAT,
        }
    }

    /// Creates a texture that can hold the image data.
    pub fn create_target_texture(
        &self,
        ctx: &RenderContext,
        texture_usages: wgpu::TextureUsages,
    ) -> GpuTexture {
        ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: self.label.clone(),
                size: wgpu::Extent3d {
                    width: self.width_height[0],
                    height: self.width_height[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1, // No mipmapping support yet.
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_texture_format(),
                usage: self.target_texture_usage_requirements() | texture_usages,
            },
        )
    }
}

/// Takes raw image data and transfers & converts it to a GPU texture.
///
/// Schedules render passes to convert the data to a samplable textures if needed.
///
/// Generally, we currently do *not* use sRGB converting formats like [`wgpu::TextureFormat::Rgba8UnormSrgb`] in order to…
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
    target_texture: &GpuTexture,
) -> Result<(), ImageDataToTextureError> {
    re_tracing::profile_function!();

    image_data.validate(&ctx.device.limits(), &target_texture.creation_desc)?;

    let ImageDataDesc {
        label,
        data,
        format: source_format,
        width_height: output_width_height,
    } = image_data;

    // Determine size of the texture the image data is uploaded into.
    // Reminder: We can't use raw buffers because of WebGL compatibility.
    let [data_texture_width, data_texture_height] = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => output_width_height,
        SourceImageDataFormat::Yuv { layout, .. } => {
            layout.data_texture_width_height(output_width_height)
        }
    };
    let data_texture_format = match source_format {
        SourceImageDataFormat::WgpuCompatible(format) => format,
        SourceImageDataFormat::Yuv { layout, .. } => layout.data_texture_format(),
    };

    // Allocate gpu belt data and upload it.
    let data_texture_label = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => label.clone(),
        SourceImageDataFormat::Yuv { .. } => format!("{label}_source_data").into(),
    };

    let data_texture = match source_format {
        // Needs intermediate data texture.
        SourceImageDataFormat::Yuv { .. } => ctx.gpu_resources.textures.alloc(
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
        ),

        // Target is directly written to.
        SourceImageDataFormat::WgpuCompatible(_) => target_texture.clone(),
    };

    copy_data_to_texture(ctx, &data_texture, data.as_ref())?;

    // Build a converter task, feeding in the raw data.
    let converter_task = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => {
            // No further conversion needed, we're done here!
            return Ok(());
        }
        SourceImageDataFormat::Yuv {
            layout,
            coefficients,
            range,
        } => YuvFormatConversionTask::new(
            ctx,
            layout,
            range,
            coefficients,
            &data_texture,
            target_texture,
        ),
    };

    // Once there's different gpu based conversions, we should probably trait-ify this so we can keep the basic steps.
    // Note that we execute the task right away, but the way things are set up (by means of using the `Renderer` framework)
    // it would be fairly easy to schedule this differently!
    converter_task
        .convert_input_data_to_texture(ctx)
        .map_err(|err| ImageDataToTextureError::GpuBasedConversionError { label, err })
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
        re_tracing::profile_scope!("bulk_copy");

        // Fast path: Just copy the data over as-is.
        gpu_read_buffer.extend_from_slice(data)?;
    } else {
        re_tracing::profile_scope!("row_by_row_copy");

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
