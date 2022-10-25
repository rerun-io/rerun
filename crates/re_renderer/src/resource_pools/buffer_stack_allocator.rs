struct BufferStackAllocation<'a> {
    staging_data: &'a mut [u8],
    binding: wgpu::BindingResource::Buffer,
}

struct BufferStackAllocator {
    open_gpu_buffer: BufferHandle,

}

impl BufferStackAllocator {
    pub fn new(usage: wgpu::BufferUsages) -> Self {}

    /// Typically done by context's frame maintenance
    pub fn reset_stack() {

    }

    pub fn finish(&mut )

    pub fn recall(&mut self);

    pub fn allocate_buffer(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &mut BufferPool,
        encoder: &mut wgpu::CommandEncoder,
        size: usize,
    ) -> BufferAllocation {
    }
}
