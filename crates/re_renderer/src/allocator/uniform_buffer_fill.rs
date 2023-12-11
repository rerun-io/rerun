use re_log::ResultExt;

pub use super::cpu_write_gpu_read_belt::{CpuWriteGpuReadBelt, CpuWriteGpuReadBuffer};

use crate::{wgpu_resources::BindGroupEntry, DebugLabel, RenderContext};

struct UniformBufferAlignmentCheck<T> {
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> UniformBufferAlignmentCheck<T> {
    /// wgpu requires uniform buffers to be aligned to up to 256 bytes.
    ///
    /// This is a property of device limits, see [`WebGPU` specification](https://www.w3.org/TR/webgpu/#limits).
    /// Implementations are allowed to advertise a lower alignment requirement, however
    /// 256 bytes is fairly common even in modern hardware and is even hardcoded for DX12.
    ///
    /// Technically this is only relevant when sub-allocating a buffer, as the wgpu backend
    /// is internally forced to make sure that the start of any [`wgpu::Buffer`] with [`wgpu::BufferUsages::UNIFORM`] usage
    /// has this alignment. Practically, ensuring this alignment everywhere
    ///
    /// Alternatively to enforcing this alignment on the type we could:
    /// * only align on the gpu buffer
    ///     -> causes more fine grained `copy_buffer_to_buffer` calls on the gpu encoder
    /// * only align on the [`CpuWriteGpuReadBuffer`] & gpu buffer
    ///     -> causes more complicated offset computation on [`CpuWriteGpuReadBuffer`] as well as either
    ///         holes at padding (-> undefined values & slow for write combined!) or complicated nulling of padding
    ///
    /// About the [`bytemuck::Pod`] requirement (dragged in by [`CpuWriteGpuReadBuffer`]):
    /// [`bytemuck::Pod`] forces us to be explicit about padding as it doesn't allow invisible padding bytes!
    /// We could drop this and thus make it easier to define uniform buffer types.
    /// But this leads to more unsafe code, harder to avoid holes in write combined memory access
    /// and potentially undefined values in the padding bytes on GPU.
    const CHECK: () = assert!(
        std::mem::align_of::<T>() >= 256 && std::mem::size_of::<T>() > 0,
        "Uniform buffers need to be bigger than 0 bytes and aligned to 256 bytes. Use `#[repr(C, align(256))]`"
    );
}

/// Utility for fast & efficient creation of uniform buffers from a series of structs.
///
/// For subsequent frames, this will automatically not allocate any resources (thanks to our buffer pooling mechanism).
///
/// TODO(#1383): We could do this on a more complex stack allocator.
pub fn create_and_fill_uniform_buffer_batch<T: bytemuck::Pod + Send + Sync>(
    ctx: &RenderContext,
    label: DebugLabel,
    content: impl ExactSizeIterator<Item = T>,
) -> Vec<BindGroupEntry> {
    re_tracing::profile_function!(label.get().unwrap_or_default());

    #[allow(clippy::let_unit_value)]
    let _ = UniformBufferAlignmentCheck::<T>::CHECK;

    if content.len() == 0 {
        return vec![];
    }

    let num_buffers = content.len() as u64;
    let element_size = std::mem::size_of::<T>() as u64;

    let buffer = ctx.gpu_resources.buffers.alloc(
        &ctx.device,
        &crate::wgpu_resources::BufferDesc {
            label,
            size: num_buffers * element_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    );

    let Some(mut staging_buffer) = ctx
        .cpu_write_gpu_read_belt
        .lock()
        .allocate::<T>(&ctx.device, &ctx.gpu_resources.buffers, num_buffers as _)
        .unwrap_debug_or_log_error()
    else {
        // This should only fail for zero sized T, which we assert statically on.
        return Vec::new();
    };
    staging_buffer.extend(content).unwrap_debug_or_log_error();
    staging_buffer
        .copy_to_buffer(
            ctx.active_frame.before_view_builder_encoder.lock().get(),
            &buffer,
            0,
        )
        .unwrap_debug_or_log_error();

    (0..num_buffers)
        .map(|i| BindGroupEntry::Buffer {
            handle: buffer.handle,
            offset: i * element_size,
            size: Some(std::num::NonZeroU64::new(element_size).unwrap()),
        })
        .collect()
}

/// See [`create_and_fill_uniform_buffer`].
pub fn create_and_fill_uniform_buffer<T: bytemuck::Pod + Send + Sync>(
    ctx: &RenderContext,
    label: DebugLabel,
    content: T,
) -> BindGroupEntry {
    create_and_fill_uniform_buffer_batch(ctx, label, std::iter::once(content))
        .into_iter()
        .next()
        .unwrap()
}
