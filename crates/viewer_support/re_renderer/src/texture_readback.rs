//! Utilities for reading back GPU texture data to the CPU.
//!
//! Provides async readback via the `GpuReadbackBelt`:
//! [`schedule_read_texture`] to request a copy and [`poll_read_texture`] to
//! retrieve the result on a subsequent frame.

use crate::allocator::GpuReadbackIdentifier;
use crate::texture_info::Texture2DBufferInfo;
use crate::{GpuReadbackError, RenderContext};

/// Result of a texture readback.
pub struct TextureReadback {
    /// Raw pixel data with GPU row padding removed.
    pub data: Vec<u8>,

    /// Width and height of the texture.
    pub extent: wgpu::Extent3d,

    /// The texture's format.
    pub format: wgpu::TextureFormat,
}

/// Metadata stored on the readback belt for async texture reads.
struct ReadbackMetadata {
    extent: wgpu::Extent3d,
    format: wgpu::TextureFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureReadbackId(GpuReadbackIdentifier);

/// Schedule an async readback of a GPU texture via the `GpuReadbackBelt`.
///
/// Returns an error if the texture doesn't have [`wgpu::TextureUsages::COPY_SRC`].
///
/// Results are retrieved with [`poll_read_texture`].
pub fn schedule_read_texture(
    ctx: &RenderContext,
    texture: &wgpu::Texture,
) -> Result<TextureReadbackId, GpuReadbackError> {
    static NEXT_GPU_READBACK_IDENTIFIER: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0xa8291af2e7dd);

    let extent = texture.size();
    let format = texture.format();
    let buffer_info = Texture2DBufferInfo::new(format, extent);

    let id = NEXT_GPU_READBACK_IDENTIFIER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut readback_buffer = ctx.gpu_readback_belt.lock().allocate(
        &ctx.device,
        &ctx.gpu_resources.buffers,
        buffer_info.buffer_size_padded,
        id,
        Box::new(ReadbackMetadata { extent, format }),
    );

    readback_buffer.read_texture2d(
        ctx.active_frame.before_view_builder_encoder.lock().get(),
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        extent,
    )?;

    Ok(TextureReadbackId(id))
}

/// Poll for a completed async texture readback scheduled with [`schedule_read_texture`].
///
/// Returns `Some` with the raw pixel data when a result is ready, `None` otherwise. The
/// returned data has GPU row padding stripped.
pub fn poll_read_texture(ctx: &RenderContext, id: TextureReadbackId) -> Option<TextureReadback> {
    ctx.gpu_readback_belt.lock().readback_next_available(
        id.0,
        |data: &[u8], metadata: Box<ReadbackMetadata>| {
            let buffer_info = Texture2DBufferInfo::new(metadata.format, metadata.extent);
            let data = buffer_info.remove_padding(data).into_owned();
            TextureReadback {
                data,
                extent: metadata.extent,
                format: metadata.format,
            }
        },
    )
}
