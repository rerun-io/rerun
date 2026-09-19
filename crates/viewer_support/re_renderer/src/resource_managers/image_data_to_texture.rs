//! For an overview of image data interpretation check `re_video`'s decoder docs!

use super::rgb8_converter::Rgb8FormatConversionTask;
use super::yuv_converter::{
    YuvFormatConversionTask, YuvMatrixCoefficients, YuvPixelLayout, YuvRange,
};
use crate::renderer::DrawError;
use crate::resource_managers::AlphaChannelUsage;
use crate::wgpu_resources::{GpuTexture, TextureDesc};
use crate::{Label, RenderContext, Texture2DBufferInfo};

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

    /// Tightly packed 8-bit RGB data with three bytes per pixel.
    Rgb8,

    /// Tightly packed 8-bit BGR data with three bytes per pixel.
    Bgr8,

    /// YUV (== `YCbCr`) formats, typically using chroma downsampling.
    ///
    /// Does not handle chroma sample locations.
    Yuv {
        layout: YuvPixelLayout,
        coefficients: YuvMatrixCoefficients,
        range: YuvRange,
    },
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
    ZeroSize(Label),

    #[error(
        "Texture {label:?} was {width}x{height}, larger than the max of {max_texture_dimension_2d}"
    )]
    TooLarge {
        label: Label,
        width: u32,
        height: u32,
        max_texture_dimension_2d: u32,
    },

    #[error(
        "Invalid data length for texture {label:?}. Expected {expected} bytes, got {actual} bytes"
    )]
    InvalidDataLength {
        label: Label,
        expected: usize,
        actual: usize,
    },

    #[error(transparent)]
    CpuWriteGpuReadError(#[from] crate::allocator::CpuWriteGpuReadError),

    #[error(transparent)]
    Renderer(#[from] crate::RendererRegistrationError),

    #[error("Texture {label:?} has a format {format:?} that data can't be transferred to!")]
    UnsupportedFormatForTransfer {
        label: Label,
        format: wgpu::TextureFormat,
    },

    #[error("Gpu-based conversion for texture {label:?} did not succeed: {err}")]
    GpuBasedConversionError { label: Label, err: DrawError },

    #[error(
        "Texture {label:?} has invalid texture usage flags: {actual_usage:?}, expected at least {required_usage:?}"
    )]
    InvalidTargetTextureUsageFlags {
        label: Label,
        actual_usage: wgpu::TextureUsages,
        required_usage: wgpu::TextureUsages,
    },

    #[error(
        "Texture {label:?} has invalid texture format: {actual_format:?}, expected {required_format:?}"
    )]
    InvalidTargetTextureFormat {
        label: Label,
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
    pub label: Label,

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

    /// Information about how the alpha channel is used, if it exists.
    pub alpha_channel_usage: AlphaChannelUsage,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl ImageDataDesc<'_> {
    /// Checks the data against the texture it is to be transferred into.
    fn validate_target_texture(
        &self,
        target_texture_desc: &TextureDesc,
    ) -> Result<(), ImageDataToTextureError> {
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

        Ok(())
    }

    /// Checks the data against the device limits.
    ///
    /// This has to pass before any gpu resources are allocated for the data.
    fn validate(&self, limits: &wgpu::Limits) -> Result<(), ImageDataToTextureError> {
        let Self {
            label,
            data,
            format,
            width_height,
            alpha_channel_usage: _,
        } = self;

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
            SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => num_pixels * 3,
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
            SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => {
                Rgb8FormatConversionTask::REQUIRED_TARGET_TEXTURE_USAGE_FLAGS
            }
            SourceImageDataFormat::Yuv { .. } => {
                YuvFormatConversionTask::REQUIRED_TARGET_TEXTURE_USAGE_FLAGS
            }
        }
    }

    /// The texture format required in order to store this image data.
    pub fn target_texture_format(&self) -> wgpu::TextureFormat {
        match self.format {
            SourceImageDataFormat::WgpuCompatible(format) => format,
            SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => {
                Rgb8FormatConversionTask::OUTPUT_FORMAT
            }
            SourceImageDataFormat::Yuv { .. } => YuvFormatConversionTask::OUTPUT_FORMAT,
        }
    }

    /// Creates a texture that can hold the image data.
    ///
    /// Fails if the data doesn't fit the device limits, in which case nothing is allocated.
    pub fn create_target_texture(
        &self,
        ctx: &RenderContext,
        texture_usages: wgpu::TextureUsages,
    ) -> Result<GpuTexture, ImageDataToTextureError> {
        self.validate(&ctx.device.limits())?;

        Ok(ctx.gpu_resources.textures.alloc(
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
        ))
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

    image_data.validate(&ctx.device.limits())?;
    image_data.validate_target_texture(&target_texture.creation_desc)?;

    let ImageDataDesc {
        label,
        data,
        format: source_format,
        width_height: output_width_height,
        alpha_channel_usage: _, // TODO(#12223): Determine `AlphaChannelUsage` if it is set to `DontKnow`.
    } = image_data;

    // Determine size of the texture the image data is uploaded into.
    // Reminder: We can't use raw buffers because of WebGL compatibility.
    let [data_texture_width, data_texture_height] = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => output_width_height,
        // Pack the raw 3-byte pixels four bytes at a time into an RGBA8Uint source texture.
        // The fragment conversion pass reconstructs RGB/BGR pixels from this byte stream.
        SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => [
            (output_width_height[0] * 3).div_ceil(4),
            output_width_height[1],
        ],
        SourceImageDataFormat::Yuv { layout, .. } => {
            layout.data_texture_width_height(output_width_height)
        }
    };
    let data_texture_format = match source_format {
        SourceImageDataFormat::WgpuCompatible(format) => format,
        SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => wgpu::TextureFormat::Rgba8Uint,
        SourceImageDataFormat::Yuv { layout, .. } => layout.data_texture_format(),
    };

    // Allocate gpu belt data and upload it.
    let data_texture_label = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => label.clone(),
        SourceImageDataFormat::Rgb8
        | SourceImageDataFormat::Bgr8
        | SourceImageDataFormat::Yuv { .. } => format!("{label}_source_data").into(),
    };

    let data_texture = match source_format {
        // Needs intermediate data texture.
        SourceImageDataFormat::Rgb8
        | SourceImageDataFormat::Bgr8
        | SourceImageDataFormat::Yuv { .. } => ctx.gpu_resources.textures.alloc(
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

    let source_bytes_per_row = match source_format {
        SourceImageDataFormat::Rgb8 | SourceImageDataFormat::Bgr8 => {
            Some(output_width_height[0] as usize * 3)
        }
        _ => None,
    };
    copy_data_to_texture(ctx, &data_texture, data.as_ref(), source_bytes_per_row)?;

    let conversion_result = match source_format {
        SourceImageDataFormat::WgpuCompatible(_) => return Ok(()),
        SourceImageDataFormat::Rgb8 => {
            Rgb8FormatConversionTask::new(ctx, false, &data_texture, target_texture)?
                .convert_input_data_to_texture(ctx)
        }
        SourceImageDataFormat::Bgr8 => {
            Rgb8FormatConversionTask::new(ctx, true, &data_texture, target_texture)?
                .convert_input_data_to_texture(ctx)
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
        )?
        .convert_input_data_to_texture(ctx),
    };

    conversion_result.map_err(|err| ImageDataToTextureError::GpuBasedConversionError { label, err })
}

fn copy_data_to_texture(
    render_ctx: &RenderContext,
    data_texture: &GpuTexture,
    data: &[u8],
    source_bytes_per_row: Option<usize>,
) -> Result<(), ImageDataToTextureError> {
    re_tracing::profile_function!();

    let buffer_info =
        Texture2DBufferInfo::new(data_texture.texture.format(), data_texture.texture.size());
    let texture_bytes_per_row = buffer_info.bytes_per_row_unpadded as usize;
    let source_bytes_per_row = source_bytes_per_row.unwrap_or(texture_bytes_per_row);
    re_log::debug_assert!(source_bytes_per_row <= texture_bytes_per_row);

    let mut cpu_write_gpu_read_belt = render_ctx.cpu_write_gpu_read_belt.lock();
    let mut gpu_read_buffer = cpu_write_gpu_read_belt.allocate::<u8>(
        &render_ctx.device,
        &render_ctx.gpu_resources.buffers,
        buffer_info.buffer_size_padded as usize,
    )?;

    if source_bytes_per_row == texture_bytes_per_row
        && buffer_info.buffer_size_padded as usize == data.len()
    {
        re_tracing::profile_scope!("bulk_copy");

        // Fast path: Just copy the data over as-is.
        gpu_read_buffer.extend_from_slice(data)?;
    } else {
        re_tracing::profile_scope!("row_by_row_copy");

        // Copy row by row, adding both source-format tail padding and wgpu row padding.
        let num_padding_bytes_per_row =
            buffer_info.bytes_per_row_padded as usize - source_bytes_per_row;
        let height = data_texture.texture.size().height as usize;
        re_log::debug_assert_eq!(data.len(), source_bytes_per_row * height);

        for row in 0..height {
            let row_start = row * source_bytes_per_row;
            gpu_read_buffer
                .extend_from_slice(&data[row_start..(row_start + source_bytes_per_row)])?;
            gpu_read_buffer.add_n(0, num_padding_bytes_per_row)?;
        }
    }

    let mut before_view_builder_encoder =
        render_ctx.active_frame.before_view_builder_encoder.lock();
    gpu_read_buffer
        .copy_to_texture2d_entire_first_layer(before_view_builder_encoder.get(), data_texture)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::time::Duration;

    use super::*;
    use crate::{poll_read_texture, schedule_read_texture};

    fn expected_rgba(data: &[u8], bgr: bool) -> Vec<u8> {
        data.chunks_exact(3)
            .flat_map(|pixel| {
                if bgr {
                    [pixel[2], pixel[1], pixel[0], 0]
                } else {
                    [pixel[0], pixel[1], pixel[2], 0]
                }
            })
            .collect()
    }

    #[test]
    fn packed_rgb8_gpu_conversion_matches_cpu_reference() {
        let mut ctx = RenderContext::new_test();
        let mut cases = Vec::new();

        for bgr in [false, true] {
            for (width, height) in [1, 2, 3, 4, 5, 7, 8, 63, 64, 65]
                .into_iter()
                .map(|width| (width, 3))
                .chain([1919, 1920, 1921].into_iter().map(|width| (width, 2)))
            {
                let num_pixels = width as usize * height as usize;
                let data = (0..num_pixels * 3)
                    .map(|i| ((i * 37 + 11) & 0xff) as u8)
                    .collect::<Vec<_>>();
                let expected = expected_rgba(&data, bgr);
                cases.push((width, height, bgr, data, expected));
            }
        }

        let mut readback_ids = Vec::with_capacity(cases.len());
        ctx.execute_test_frame(|ctx| {
            for (width, height, bgr, data, _) in &cases {
                let image_data = ImageDataDesc {
                    label: format!("packed_rgb8_{width}x{height}_{bgr}").into(),
                    data: Cow::Borrowed(data),
                    format: if *bgr {
                        SourceImageDataFormat::Bgr8
                    } else {
                        SourceImageDataFormat::Rgb8
                    },
                    width_height: [*width, *height],
                    alpha_channel_usage: AlphaChannelUsage::Opaque,
                };
                let texture = image_data
                    .create_target_texture(ctx, wgpu::TextureUsages::COPY_SRC)
                    .unwrap();

                transfer_image_data_to_texture(ctx, image_data, &texture).unwrap();
                readback_ids.push(schedule_read_texture(ctx, &texture.texture).unwrap());
            }
            std::iter::empty::<wgpu::CommandBuffer>()
        });

        ctx.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();

        for ((width, height, _, _, expected), readback_id) in cases.iter().zip(readback_ids) {
            let readback = poll_read_texture(&ctx, readback_id)
                .expect("GPU readback should be available after waiting for the device");
            assert_eq!(readback.format, wgpu::TextureFormat::Rgba8Unorm);
            assert_eq!(readback.extent.width, *width);
            assert_eq!(readback.extent.height, *height);
            assert_eq!(readback.data, *expected);
        }
    }
}
