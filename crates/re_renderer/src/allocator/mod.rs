//! High level GPU memory allocators.
//!
//! In contrast to the buffer pools in [`crate::wgpu_resources`], every allocator in here
//! follows some more complex strategy for efficient re-use and sub-allocation of wgpu resources.

mod cpu_write_gpu_read_belt;

pub use cpu_write_gpu_read_belt::{CpuWriteGpuReadBelt, CpuWriteGpuReadBuffer};

use crate::{wgpu_resources::GpuBuffer, DebugLabel, RenderContext};

/// Utility for fast & efficient creation of a uniform buffer from a struct.
///
/// For subsequent frames, this will automatically not allocate any resources (thanks to our buffer pooling mechanism).
/// TODO(#1383): We could do this on a more complex stack allocator.
pub fn create_and_fill_uniform_buffer<T: bytemuck::Pod>(
    ctx: &mut RenderContext,
    label: DebugLabel,
    content: T,
) -> GpuBuffer {
    let buffer = ctx.gpu_resources.buffers.alloc(
        &ctx.device,
        &crate::wgpu_resources::BufferDesc {
            label,
            size: std::mem::size_of_val(&content) as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    );

    let mut staging_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<T>(
        &ctx.device,
        &mut ctx.gpu_resources.buffers,
        1,
    );
    staging_buffer.push(content);
    staging_buffer.copy_to_buffer(
        ctx.active_frame.frame_global_command_encoder(&ctx.device),
        &buffer,
        0,
    );

    buffer
}
