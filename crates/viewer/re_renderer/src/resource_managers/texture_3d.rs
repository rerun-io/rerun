//! Creation & data upload of 3D textures, e.g. for volumes such as 3D scans.

use crate::texture_info::Texture3DBufferInfo;
use crate::wgpu_resources::{GpuTexture, GpuTextureHandle, TextureDesc};
use crate::{Label, RenderContext};

/// Handle to a 3D texture resource.
///
/// Like [`super::GpuTexture2D`], this is solely a more strongly typed regular gpu texture handle.
#[derive(Clone)]
pub struct GpuTexture3D {
    texture: GpuTexture,
}

impl std::fmt::Debug for GpuTexture3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { texture } = self;
        f.debug_struct("GpuTexture3D")
            .field("handle", &texture.handle)
            .field("size", &texture.texture.size())
            .field("format", &texture.texture.format())
            .finish()
    }
}

impl GpuTexture3D {
    /// Returns `None` if the `texture` is not 3D.
    pub fn new(texture: GpuTexture) -> Option<Self> {
        if texture.texture.dimension() != wgpu::TextureDimension::D3 {
            return None;
        }

        Some(Self { texture })
    }

    #[inline]
    pub fn handle(&self) -> GpuTextureHandle {
        self.texture.handle
    }

    /// Width, height and depth of the texture, in voxels.
    #[inline]
    pub fn dimensions(&self) -> [u32; 3] {
        let size = self.texture.texture.size();
        [size.width, size.height, size.depth_or_array_layers]
    }

    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.texture.format()
    }
}

impl AsRef<GpuTexture> for GpuTexture3D {
    #[inline(always)]
    fn as_ref(&self) -> &GpuTexture {
        &self.texture
    }
}

impl std::ops::Deref for GpuTexture3D {
    type Target = GpuTexture;

    #[inline(always)]
    fn deref(&self) -> &GpuTexture {
        &self.texture
    }
}

impl std::borrow::Borrow<GpuTexture> for GpuTexture3D {
    #[inline(always)]
    fn borrow(&self) -> &GpuTexture {
        &self.texture
    }
}

/// Error that can occur when uploading 3D texture data.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Texture3DDataError {
    #[error("Texture {0:?} has a zero dimension!")]
    ZeroSize(Label),

    #[error(
        "Texture {label:?} was {size:?}, larger than the max 3D texture size of {max_texture_dimension_3d}"
    )]
    TooLarge {
        label: Label,
        size: [u32; 3],
        max_texture_dimension_3d: u32,
    },

    #[error(
        "Texture {label:?} needs {required_bytes} bytes of staging memory, more than the max buffer size of {max_buffer_size}"
    )]
    TooLargeForStaging {
        label: Label,
        required_bytes: u64,
        max_buffer_size: u64,
    },

    #[error(
        "Invalid data length for texture {label:?}. Expected {expected} bytes, got {actual} bytes"
    )]
    InvalidDataLength {
        label: Label,
        expected: usize,
        actual: usize,
    },

    /// Block compressed formats are 2D only, and depth/stencil formats can't be 3D either.
    #[error("Texture {label:?} has a format {format:?} that can't be used for a 3D texture")]
    UnsupportedFormat {
        label: Label,
        format: wgpu::TextureFormat,
    },

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

    #[error("Texture {label:?} has size {actual_size:?}, expected {required_size:?}")]
    InvalidTargetTextureSize {
        label: Label,
        actual_size: [u32; 3],
        required_size: [u32; 3],
    },

    #[error(transparent)]
    CpuWriteGpuReadError(#[from] crate::allocator::CpuWriteGpuReadError),
}

/// Describes volume data for the purpose of creating a 3D texture.
///
/// Unlike [`super::ImageDataDesc`], no gpu-side format conversion is performed:
/// the data has to be in a format the GPU can sample directly.
///
/// [`wgpu::TextureFormat::R16Float`] is the format to reach for when displaying scalar volumes:
/// it can be sampled with trilinear filtering everywhere, whereas
/// [`wgpu::TextureFormat::R32Float`] requires the optional `FLOAT32_FILTERABLE` feature and
/// twice the memory. Use [`Texture3DDataDesc::f32_as_f16`] to convert f32 source data.
pub struct Texture3DDataDesc<'a> {
    /// Label used for the target texture, and for any intermediate resources.
    pub label: Label,

    /// Data for mip level 0, tightly packed, X varying fastest and Z slowest.
    ///
    /// I.e. it is *not* padded according to wgpu buffer->texture transfer rules,
    /// padding will happen on the fly if necessary.
    pub data: std::borrow::Cow<'a, [u8]>,

    /// Format of the data, which is also the format of the resulting texture.
    pub format: wgpu::TextureFormat,

    /// Width, height and depth of the volume, in voxels.
    pub dimensions: [u32; 3],
}

impl Texture3DDataDesc<'_> {
    /// Describes a [`wgpu::TextureFormat::R16Float`] volume, converting `values` to f16.
    ///
    /// Values outside of the f16 range end up as infinities, and precision is roughly 3 decimal
    /// digits. That is plenty for display of a scan, but a poor fit for data that needs the
    /// exact f32 values, e.g. a signed distance field.
    pub fn f32_as_f16(label: Label, values: &[f32], dimensions: [u32; 3]) -> Self {
        re_tracing::profile_function!();

        let values = values
            .iter()
            .map(|&value| half::f16::from_f32(value))
            .collect::<Vec<_>>();

        Self {
            label,
            data: bytemuck::cast_slice(&values).to_vec().into(),
            format: wgpu::TextureFormat::R16Float,
            dimensions,
        }
    }

    /// The texture usages required in order to store this data.
    pub const REQUIRED_TARGET_TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::COPY_DST;

    fn extent(&self) -> wgpu::Extent3d {
        let [width, height, depth] = self.dimensions;
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        }
    }

    /// Checks the data against the texture it is to be copied into.
    fn validate_target_texture(
        &self,
        target_texture_desc: &TextureDesc,
    ) -> Result<(), Texture3DDataError> {
        let Self {
            label: _,
            data: _,
            format,
            dimensions,
        } = self;

        if !target_texture_desc
            .usage
            .contains(Self::REQUIRED_TARGET_TEXTURE_USAGE)
        {
            return Err(Texture3DDataError::InvalidTargetTextureUsageFlags {
                label: target_texture_desc.label.clone(),
                actual_usage: target_texture_desc.usage,
                required_usage: Self::REQUIRED_TARGET_TEXTURE_USAGE,
            });
        }
        if target_texture_desc.format != *format {
            return Err(Texture3DDataError::InvalidTargetTextureFormat {
                label: target_texture_desc.label.clone(),
                actual_format: target_texture_desc.format,
                required_format: *format,
            });
        }
        let target_size = target_texture_desc.size;
        if target_size != self.extent() {
            return Err(Texture3DDataError::InvalidTargetTextureSize {
                label: target_texture_desc.label.clone(),
                actual_size: [
                    target_size.width,
                    target_size.height,
                    target_size.depth_or_array_layers,
                ],
                required_size: *dimensions,
            });
        }

        Ok(())
    }

    /// Checks the data against the device limits.
    ///
    /// This has to pass before any gpu resources are allocated for the data.
    fn validate(&self, limits: &wgpu::Limits) -> Result<(), Texture3DDataError> {
        let Self {
            label,
            data,
            format,
            dimensions,
        } = self;

        if dimensions.contains(&0) {
            return Err(Texture3DDataError::ZeroSize(label.clone()));
        }

        // All three dimensions of a 3D texture share a single, much lower limit than 2D textures do.
        let max_texture_dimension_3d = limits.max_texture_dimension_3d;
        if dimensions.iter().any(|&dim| dim > max_texture_dimension_3d) {
            return Err(Texture3DDataError::TooLarge {
                label: label.clone(),
                size: *dimensions,
                max_texture_dimension_3d,
            });
        }

        // Neither block compressed nor depth/stencil formats can be used for 3D textures.
        // Multi-texel blocks catch the former, `is_depth_stencil_format` the latter
        // (`block_copy_size` returns `None` for some, but not all, depth/stencil formats).
        if format.is_depth_stencil_format() || format.block_dimensions() != (1, 1) {
            return Err(Texture3DDataError::UnsupportedFormat {
                label: label.clone(),
                format: *format,
            });
        }
        let Some(block_size) = format.block_copy_size(None) else {
            return Err(Texture3DDataError::UnsupportedFormat {
                label: label.clone(),
                format: *format,
            });
        };

        let num_voxels = dimensions[0] as usize * dimensions[1] as usize * dimensions[2] as usize;
        let expected_num_bytes = num_voxels * block_size as usize;
        if data.len() != expected_num_bytes {
            return Err(Texture3DDataError::InvalidDataLength {
                label: label.clone(),
                expected: expected_num_bytes,
                actual: data.len(),
            });
        }

        // The data has to pass through a single staging buffer, which is bounded independently of the texture size.
        let required_bytes = Texture3DBufferInfo::new(*format, self.extent()).buffer_size_padded;
        if required_bytes > limits.max_buffer_size {
            return Err(Texture3DDataError::TooLargeForStaging {
                label: label.clone(),
                required_bytes,
                max_buffer_size: limits.max_buffer_size,
            });
        }

        Ok(())
    }

    /// Creates a texture that can hold this data.
    ///
    /// Fails if the data doesn't fit the device limits, in which case nothing is allocated.
    pub fn create_target_texture(
        &self,
        ctx: &RenderContext,
        texture_usages: wgpu::TextureUsages,
    ) -> Result<GpuTexture3D, Texture3DDataError> {
        self.validate(&ctx.device.limits())?;

        let texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: self.label.clone(),
                size: self.extent(),
                mip_level_count: 1, // No mipmapping support yet.
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: self.format,
                usage: Self::REQUIRED_TARGET_TEXTURE_USAGE | texture_usages,
            },
        );

        Ok(GpuTexture3D::new(texture).expect("Texture is known to be 3D"))
    }
}

/// Takes raw volume data and transfers it to a 3D GPU texture.
///
/// Like the 2D path, the data goes through a `CpuWriteGpuReadBelt` buffer, since a
/// buffer->texture copy is the only way to get data into a texture on WebGL.
/// Note that this means the volume is held twice in GPU-visible memory until the copy is done.
pub fn transfer_texture_3d_data(
    ctx: &RenderContext,
    data_desc: &Texture3DDataDesc<'_>,
    target_texture: &GpuTexture3D,
) -> Result<(), Texture3DDataError> {
    re_tracing::profile_function!();

    data_desc.validate(&ctx.device.limits())?;
    data_desc.validate_target_texture(&target_texture.creation_desc)?;

    let buffer_info = Texture3DBufferInfo::new(data_desc.format, data_desc.extent());
    let data = data_desc.data.as_ref();

    let mut cpu_write_gpu_read_belt = ctx.cpu_write_gpu_read_belt.lock();
    let mut gpu_read_buffer = cpu_write_gpu_read_belt.allocate::<u8>(
        &ctx.device,
        &ctx.gpu_resources.buffers,
        buffer_info.buffer_size_padded as usize,
    )?;

    let bytes_per_row_unpadded = buffer_info.slice.bytes_per_row_unpadded as usize;
    let num_padding_bytes_per_row =
        buffer_info.slice.bytes_per_row_padded as usize - bytes_per_row_unpadded;

    if num_padding_bytes_per_row == 0 {
        re_tracing::profile_scope!("bulk_copy");

        // Fast path: Just copy the data over as-is.
        gpu_read_buffer.extend_from_slice(data)?;
    } else {
        re_tracing::profile_scope!("row_by_row_copy");

        // Copy row by row in order to jump over padding bytes.
        // Rows are padded individually, slices are not: a slice's rows are all the same size.
        let num_rows = buffer_info.rows_per_image as usize * buffer_info.depth as usize;
        for row in 0..num_rows {
            let offset = row * bytes_per_row_unpadded;
            gpu_read_buffer.extend_from_slice(&data[offset..(offset + bytes_per_row_unpadded)])?;
            gpu_read_buffer.add_n(0, num_padding_bytes_per_row)?;
        }
    }

    let mut before_view_builder_encoder = ctx.active_frame.before_view_builder_encoder.lock();
    gpu_read_buffer
        .copy_to_texture3d_entire_mip0(before_view_builder_encoder.get(), target_texture)?;

    Ok(())
}

/// Creates a 3D texture and schedules the upload of `data_desc`'s data into it.
pub fn create_and_upload_texture_3d(
    ctx: &RenderContext,
    data_desc: &Texture3DDataDesc<'_>,
) -> Result<GpuTexture3D, Texture3DDataError> {
    let texture = data_desc.create_target_texture(ctx, wgpu::TextureUsages::TEXTURE_BINDING)?;
    transfer_texture_3d_data(ctx, data_desc, &texture)?;
    Ok(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uploads a volume and reads it back, checking that every voxel survived the round trip.
    ///
    /// The width is picked so that a row is far shorter than [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`],
    /// exercising the row-by-row padding path, and the depth is >1 so that a wrong
    /// `rows_per_image` would show up as shifted slices.
    #[test]
    fn upload_and_read_back_volume() {
        let dimensions = [3, 5, 2];
        let [width, height, depth] = dimensions;
        let num_voxels = (width * height * depth) as usize;

        // Integers up to 2048 are exact in f16, so the round trip should be lossless here.
        let voxels = (0..num_voxels)
            .map(|i| half::f16::from_f32(i as f32))
            .collect::<Vec<_>>();

        let mut ctx = RenderContext::new_test();
        let buffer_info = Texture3DBufferInfo::new(
            wgpu::TextureFormat::R16Float,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
        );
        assert_eq!(buffer_info.slice.bytes_per_row_unpadded, width * 2);
        assert_eq!(
            buffer_info.slice.bytes_per_row_padded,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        );

        let readback_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume readback"),
            size: buffer_info.buffer_size_padded,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ctx.execute_test_frame(|ctx| {
            let values = voxels.iter().map(|v| v.to_f32()).collect::<Vec<f32>>();
            let data_desc =
                Texture3DDataDesc::f32_as_f16("test volume".into(), &values, dimensions);
            let texture = data_desc
                .create_target_texture(
                    ctx,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                )
                .expect("Failed to create the target texture");
            transfer_texture_3d_data(ctx, &data_desc, &texture).expect("Failed to upload volume");
            assert_eq!(texture.dimensions(), dimensions);
            assert_eq!(texture.format(), wgpu::TextureFormat::R16Float);

            let gpu_texture: &GpuTexture = texture.as_ref();
            let mut encoder = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("volume readback"),
                });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback_buffer,
                    layout: buffer_info.buffer_layout(0),
                },
                gpu_texture.texture.size(),
            );
            [encoder.finish()]
        });

        readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| {});
        ctx.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("Failed to wait for readback");

        let read_back = {
            let mapped = readback_buffer
                .slice(..)
                .get_mapped_range()
                .expect("Failed to map readback buffer");

            // Each slice is padded on its own, so unpad them one by one.
            let bytes_per_slice = buffer_info.slice.buffer_size_padded as usize;
            (0..depth as usize)
                .flat_map(|z| {
                    buffer_info.slice.remove_padding_and_convert::<half::f16>(
                        &mapped[(z * bytes_per_slice)..((z + 1) * bytes_per_slice)],
                    )
                })
                .collect::<Vec<half::f16>>()
        };

        assert_eq!(read_back, voxels);
    }

    #[test]
    fn wrong_data_length_is_an_error() {
        let ctx = RenderContext::new_test();

        let err = create_and_upload_texture_3d(
            &ctx,
            &Texture3DDataDesc {
                label: "too little data".into(),
                data: vec![0; 4].into(),
                format: wgpu::TextureFormat::R32Float,
                dimensions: [2, 2, 2],
            },
        )
        .expect_err("Expected an error for too little data");

        assert!(
            matches!(
                err,
                Texture3DDataError::InvalidDataLength {
                    expected: 32,
                    actual: 4,
                    ..
                }
            ),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn depth_formats_are_rejected() {
        let ctx = RenderContext::new_test();

        // `Depth16Unorm` has a single-texel block with a well-defined copy size,
        // so only an explicit depth/stencil check rejects it.
        let err = create_and_upload_texture_3d(
            &ctx,
            &Texture3DDataDesc {
                label: "depth".into(),
                data: vec![0; 16].into(),
                format: wgpu::TextureFormat::Depth16Unorm,
                dimensions: [2, 2, 2],
            },
        )
        .expect_err("Expected an error for a depth format");

        assert!(
            matches!(err, Texture3DDataError::UnsupportedFormat { .. }),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn compressed_formats_are_rejected() {
        let ctx = RenderContext::new_test();

        let err = create_and_upload_texture_3d(
            &ctx,
            &Texture3DDataDesc {
                label: "block compressed".into(),
                data: vec![0; 16].into(),
                format: wgpu::TextureFormat::Bc1RgbaUnorm,
                dimensions: [4, 4, 1],
            },
        )
        .expect_err("Expected an error for a block compressed format");

        assert!(
            matches!(err, Texture3DDataError::UnsupportedFormat { .. }),
            "Unexpected error: {err}"
        );
    }
}
