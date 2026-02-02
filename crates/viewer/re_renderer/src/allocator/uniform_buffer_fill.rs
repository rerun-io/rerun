use re_log::ResultExt as _;

use crate::wgpu_resources::BindGroupEntry;
use crate::{DebugLabel, RenderContext};

struct UniformBufferSizeCheck<T> {
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> UniformBufferSizeCheck<T> {
    /// wgpu requires uniform buffers to be aligned to up to 256 bytes.
    ///
    /// By ensuring that all uniform buffers have a size that is a multiple of 256 bytes,
    /// we are guaranteed that bulk copies of multiple uniform buffers in a cpu-write-gpu-read buffer
    /// can be copied to (a 256 byte aligned) gpu-readable buffer in a single copy operation.
    ///
    /// This requirement is a property of device limits, see [`WebGPU` specification](https://www.w3.org/TR/webgpu/#limits).
    /// Implementations are allowed to advertise a lower alignment requirement, however
    /// 256 bytes is fairly common even in modern hardware and is hardcoded to this value for DX12.
    ///
    /// About the [`bytemuck::Pod`] requirement (dragged in by [`CpuWriteGpuReadBuffer`][crate::allocator::CpuWriteGpuReadBuffer]):
    /// [`bytemuck::Pod`] forces us to be explicit about padding as it doesn't allow invisible padding bytes!
    /// We could drop this and thus make it easier to define uniform buffer types.
    /// But this leads to more unsafe code, harder to avoid holes in write combined memory access
    /// and potentially undefined values in the padding bytes on GPU.
    const CHECK: () = assert!(
        std::mem::size_of::<T>().is_multiple_of(256) && std::mem::size_of::<T>() > 0,
        "Uniform buffers need to have a size that is a multiple of 256 bytes.
 Use types like `F32RowPadded` or `PaddingRow` to pad out as needed."
    );
}

/// Utility for fast & efficient creation of uniform buffers from a series of structs.
///
/// For subsequent frames, this will automatically not allocate any resources (thanks to our buffer pooling mechanism).
pub fn create_and_fill_uniform_buffer_batch<T: bytemuck::Pod + Send + Sync>(
    ctx: &RenderContext,
    label: DebugLabel,
    content: impl ExactSizeIterator<Item = T>,
) -> Vec<BindGroupEntry> {
    re_tracing::profile_function!(label.get().unwrap_or_default());

    #[expect(clippy::let_unit_value)]
    let _ = UniformBufferSizeCheck::<T>::CHECK;

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
        .ok_or_log_error()
    else {
        // This should only fail for zero sized T, which we assert statically on.
        return Vec::new();
    };
    staging_buffer.extend(content).ok_or_log_error();
    staging_buffer
        .copy_to_buffer(
            ctx.active_frame.before_view_builder_encoder.lock().get(),
            &buffer,
            0,
        )
        .ok_or_log_error();

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
