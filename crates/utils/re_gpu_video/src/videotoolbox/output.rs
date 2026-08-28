//! Decoded frames as wgpu textures.
//!
//! `VideoToolbox` hands out `CVPixelBuffer`s backed by an `IOSurface`. Metal wraps each
//! plane of that surface as an `MTLTexture` without copying anything, and those go to
//! wgpu through `texture_from_raw`/`create_texture_from_hal`. The frame keeps the pixel
//! buffer retained, so `VideoToolbox`'s buffer pool can't hand the same surface to a
//! later frame while this one is still on screen.

use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetHeight, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferGetIOSurface, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferGetWidthOfPlane, kCVImageBufferYCbCrMatrix_ITU_R_601_4,
    kCVImageBufferYCbCrMatrix_ITU_R_709_2, kCVImageBufferYCbCrMatrixKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_metal::{
    MTLDevice as _, MTLPixelFormat, MTLStorageMode, MTLTextureDescriptor, MTLTextureType,
    MTLTextureUsage,
};

use objc2_core_foundation::CFString;

use crate::{ColorProperties, DecodeError, DecodedFrame, MatrixCoefficients};

use super::PixelBuffer;

/// Turns one decoded pixel buffer into a frame holding a texture per NV12 plane.
pub(super) fn wrap(
    wgpu_device: &wgpu::Device,
    pixel_buffer: &PixelBuffer,
    pts: i64,
    is_idr: bool,
) -> Result<DecodedFrame, DecodeError> {
    re_tracing::profile_function!();

    let buffer: &CVPixelBuffer = pixel_buffer.get();

    let color = color_properties(buffer)?;

    // The display size, already cropped by `VideoToolbox`. The planes can be larger.
    let width = CVPixelBufferGetWidth(buffer) as u32;
    let height = CVPixelBufferGetHeight(buffer) as u32;

    let Some(surface) = CVPixelBufferGetIOSurface(Some(buffer)) else {
        return Err(DecodeError::TextureImport(
            "the decoded pixel buffer is not backed by an IOSurface",
        ));
    };

    #[expect(unsafe_code)]
    // SAFETY: `as_hal` hands out the hal device of this wgpu device, which the video
    // context only exists for on Metal. The textures below are created on that same
    // device, are wrapped with matching descriptors, and their memory (the IOSurface)
    // outlives them: the drop callbacks keep the pixel buffer retained.
    unsafe {
        let hal_device =
            wgpu_device
                .as_hal::<wgpu::hal::api::Metal>()
                .ok_or(DecodeError::TextureImport(
                    "the video context only exists for Metal devices",
                ))?;
        let mtl_device = hal_device.raw_device();

        // IOSurface-backed textures can't be private: shared memory on Apple silicon,
        // managed on the Intel Macs where the GPU has its own memory.
        let storage_mode = if mtl_device.hasUnifiedMemory() {
            MTLStorageMode::Shared
        } else {
            MTLStorageMode::Managed
        };

        let plane = |index: usize, format, mtl_format| -> Result<_, DecodeError> {
            let plane_width = CVPixelBufferGetWidthOfPlane(buffer, index) as u32;
            let plane_height = CVPixelBufferGetHeightOfPlane(buffer, index) as u32;

            let descriptor = MTLTextureDescriptor::new();
            descriptor.setTextureType(MTLTextureType::Type2D);
            descriptor.setPixelFormat(mtl_format);
            descriptor.setWidth(plane_width as usize);
            descriptor.setHeight(plane_height as usize);
            descriptor.setMipmapLevelCount(1);
            descriptor.setStorageMode(storage_mode);
            descriptor.setUsage(MTLTextureUsage::ShaderRead);

            let mtl_texture = mtl_device
                .newTextureWithDescriptor_iosurface_plane(&descriptor, &surface, index)
                .ok_or(DecodeError::TextureImport(
                    "Metal refused to wrap the IOSurface plane as a texture",
                ))?;

            let size = wgpu::Extent3d {
                width: plane_width,
                height: plane_height,
                depth_or_array_layers: 1,
            };
            // Retained per plane so the pixel buffer outlives whichever texture
            // wgpu destroys last.
            let keep_alive = pixel_buffer.clone();
            let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
                mtl_texture,
                format,
                MTLTextureType::Type2D,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width: plane_width,
                    height: plane_height,
                    depth: 1,
                },
                Some(Box::new(move || drop(keep_alive))),
            );

            Ok(
                wgpu_device.create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal_texture,
                    &wgpu::TextureDescriptor {
                        label: Some("video decode output"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::TextureUses::RESOURCE,
                ),
            )
        };

        let y_texture = plane(0, wgpu::TextureFormat::R8Unorm, MTLPixelFormat::R8Unorm)?;
        let uv_texture = plane(1, wgpu::TextureFormat::Rg8Unorm, MTLPixelFormat::RG8Unorm)?;
        drop(hal_device);

        let y = y_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("video decode output (luma)"),
            ..Default::default()
        });
        let uv = uv_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("video decode output (chroma)"),
            ..Default::default()
        });

        Ok(DecodedFrame::new_planar(
            y_texture, uv_texture, y, uv, width, height, pts, is_idr, color,
        ))
    }
}

/// Reads the sample range from the pixel format and the matrix from the buffer's attachments.
fn color_properties(buffer: &CVPixelBuffer) -> Result<ColorProperties, DecodeError> {
    let pixel_format = CVPixelBufferGetPixelFormatType(buffer);
    let full_range = if pixel_format == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange {
        false
    } else if pixel_format == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange {
        true
    } else {
        return Err(DecodeError::TextureImport(
            "VideoToolbox decoded the stream to something other than 8 bit NV12",
        ));
    };

    #[expect(unsafe_code)]
    // SAFETY: The key is one of CoreVideo's own attachment keys, and a null mode
    // pointer means the attachment mode is not reported back.
    let matrix = unsafe { buffer.attachment(kCVImageBufferYCbCrMatrixKey, std::ptr::null_mut()) };

    #[expect(unsafe_code)]
    // SAFETY: CoreVideo's own matrix constants, valid for the process's lifetime.
    let (bt709, bt601) = unsafe {
        (
            kCVImageBufferYCbCrMatrix_ITU_R_709_2,
            kCVImageBufferYCbCrMatrix_ITU_R_601_4,
        )
    };
    let matrix_coefficients = match matrix
        .as_deref()
        .and_then(|matrix| matrix.downcast_ref::<CFString>())
    {
        Some(matrix) if matrix == bt709 => MatrixCoefficients::Bt709,
        Some(matrix) if matrix == bt601 => MatrixCoefficients::Bt601,
        _ => MatrixCoefficients::Unspecified,
    };

    Ok(ColorProperties {
        full_range,
        matrix_coefficients,
    })
}
