//! High level GPU memory allocators.
//!
//! In contrast to the buffer pools in [`crate::wgpu_resources`], every allocator in here
//! follows some more complex strategy for efficient re-use and sub-allocation of wgpu resources.

mod cpu_write_gpu_read_belt;

pub use cpu_write_gpu_read_belt::{CpuWriteGpuReadBelt, CpuWriteGpuReadBuffer};

use crate::{wgpu_resources::BindGroupEntry, DebugLabel, RenderContext};

/// Utility for fast & efficient creation of uniform buffers from a series of structs.
///
/// For subsequent frames, this will automatically not allocate any resources (thanks to our buffer pooling mechanism).
///
/// TODO(#1383): We could do this on a more complex stack allocator.
pub fn create_and_fill_uniform_buffer_batch<T: bytemuck::Pod>(
    ctx: &mut RenderContext,
    label: DebugLabel,
    content: impl ExactSizeIterator<Item = T>,
) -> Vec<BindGroupEntry> {
    let num_buffers = content.len() as u64;
    let element_size = std::mem::size_of::<T>() as u64;

    assert!(
        element_size > 0,
        "Uniform buffer need to have a non-zero size"
    );
    assert!(
        std::mem::align_of::<T>() % 16 == 0,
        "Uniform buffer size needs to be aligned to 4xf32, i.e. 16 bytes"
    );

    let buffer = ctx.gpu_resources.buffers.alloc(
        &ctx.device,
        &crate::wgpu_resources::BufferDesc {
            label,
            size: num_buffers * element_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    );

    let mut staging_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<T>(
        &ctx.device,
        &mut ctx.gpu_resources.buffers,
        1,
    );
    staging_buffer.extend(content);
    staging_buffer.copy_to_buffer(
        ctx.active_frame.frame_global_command_encoder(&ctx.device),
        &buffer,
        0,
    );

    (0..num_buffers)
        .into_iter()
        .map(|i| BindGroupEntry::Buffer {
            handle: buffer.handle,
            offset: i * element_size,
            size: Some(std::num::NonZeroU64::new(element_size).unwrap()),
        })
        .collect()
}

/// See [`create_and_fill_uniform_buffer`].
pub fn create_and_fill_uniform_buffer<T: bytemuck::Pod>(
    ctx: &mut RenderContext,
    label: DebugLabel,
    content: T,
) -> BindGroupEntry {
    create_and_fill_uniform_buffer_batch(ctx, label, std::iter::once(content))
        .into_iter()
        .next()
        .unwrap()
}
