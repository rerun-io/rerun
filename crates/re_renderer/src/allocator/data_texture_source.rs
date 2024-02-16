use bytemuck::Pod;

use crate::{
    wgpu_resources::{self, GpuTexture},
    DebugLabel, RenderContext,
};

use super::{CpuWriteGpuReadBuffer, CpuWriteGpuReadError};

/// Utility for writing data to a dynamically sized "data textures".
///
/// For WebGL compatibility we sometimes have to use textures instead of buffers.
/// We call these textures "data textures".
/// This construct allows to write data directly to gpu readable memory which
/// then upon finishing is automatically copied into an appropriately sized
/// texture which receives all data written to [`DataTextureSource`].
/// Each texel in the data texture represents a single element of the type `T`.
pub struct DataTextureSource<'a, T: Pod + Send + Sync> {
    ctx: &'a RenderContext, // TODO(andreas): take more fine-grained reference?

    /// Buffers that need to be transferred to the data texture in the end.
    ///
    /// We have two options on how to deal with buffer allocation and filling:
    ///
    /// 1. fill the last buffer to its maximum capacity before starting writing to the next,
    ///    allow arbitrary amount of empty buffers
    ///      -> Pro: makes the final gpu copies easy, we don't have to juggle weird offsets and several copies per buffer!
    ///      -> Con: may need to spread writes over several buffers
    /// 2. create a new buffer whenever a write doesn't fully fit into the active buffer,
    ///    even if the last buffer has some remaining capacity, allow arbitrary amount of half-filled buffers
    ///      -> Pro: All writes can go to a single buffer, which is simpler & faster.
    ///      -> Con: We waste space and copying to the texture is harder
    ///
    /// We're going with option (1)!
    ///
    /// This means that there might be 'n' full buffers followed by a single active buffer, followed by 'm' empty buffers.
    buffers: Vec<CpuWriteGpuReadBuffer<T>>,

    /// Buffer in which data is currently written.
    ///
    /// All buffers before are full and all after (if any) are empty.
    ///
    /// After `reserve` it is guaranteed to point to a buffer with remaining capacity.
    /// Prior to that, it may point to no buffer at all or a full buffer.
    active_buffer_index: usize,
}

impl<'a, T: Pod + Send + Sync> DataTextureSource<'a, T> {
    /// Creates a new `DataTextureSource` with the given `RenderContext`.
    ///
    /// This operation itself will not allocate any memory, empty `DataTextureSource` are not a concern.
    pub fn new(ctx: &'a RenderContext) -> Self {
        Self {
            ctx,
            buffers: Vec::new(),
            active_buffer_index: 0,
        }
    }

    #[inline]
    fn max_texture_width(&self) -> usize {
        // We limit the data texture width to 32768 or whatever smaller value is supported.
        // (in fact, more commonly supported values are 8192 and 16384)
        //
        // This then means that if we're always a multiple of this width,
        // we can do all buffer copies in a single copy!
        //
        // But wait! Isn't this too big for a minimum size?
        // 32768 * float4 (worst case) = 0.5MiB.
        // Yes, not nothing, but also not all that much, and keep in mind that weaker hardware will have 8192 max width.
        // Note also, that many of our textures use 4 & 8 byte formats.
        (self.ctx.device.limits().max_texture_dimension_2d as usize).min(32768)
    }

    /// The number of elements written so far.
    #[inline]
    pub fn num_written(&self) -> usize {
        self.buffers.iter().map(|b| b.num_written()).sum()
    }

    /// Reserves space for at least `num_elements` elements.
    ///
    /// Creating new buffers is a relatively expensive operation, so it's best to
    /// reserve gratuitously!
    /// Ideally, you know exactly how many elements you're going to write and reserve
    /// accordingly at the start.
    pub fn reserve(&mut self, num_elements: usize) -> Result<(), CpuWriteGpuReadError> {
        let remaining_capacity: usize = self.buffers.iter().map(|b| b.remaining_capacity()).sum();
        if remaining_capacity >= num_elements {
            return Ok(());
        }

        // Constraints on the buffer size:
        // * have at least as many elements as requested
        // * be at least double the size of the last buffer
        // * keep it easy to copy to textures by always being a multiple of the maximum row size we use for data textures
        //      -> this massively simplifies the buffer->texture copy logic!
        let last_buffer_size = self.buffers.last().map_or(0, |b| b.capacity());
        let new_buffer_size = (num_elements - remaining_capacity)
            .max(last_buffer_size * 2)
            .next_multiple_of(self.max_texture_width());

        self.buffers
            .push(self.ctx.cpu_write_gpu_read_belt.lock().allocate(
                &self.ctx.device,
                &self.ctx.gpu_resources.buffers,
                new_buffer_size,
            )?);

        // Ensure invariant that the active buffer has some remaining capacity.
        if self.buffers[self.active_buffer_index].remaining_capacity() == 0 {
            self.active_buffer_index += 1;
        }

        Ok(())
    }

    #[inline]
    pub fn fill_n(&mut self, element: T, num_elements: usize) -> Result<(), CpuWriteGpuReadError> {
        self.reserve(num_elements)?;

        let mut num_elements_remaining = num_elements;

        loop {
            let result =
                self.buffers[self.active_buffer_index].fill_n(element, num_elements_remaining);

            // `fill_n` is documented to write as many elements as possible, so we can just continue with the next buffer,
            // if we ran out of space!
            if let Err(CpuWriteGpuReadError::BufferFull {
                buffer_capacity_elements,
                buffer_size_elements,
                num_elements_attempted_to_add: _,
            }) = result
            {
                let actually_written = buffer_capacity_elements - buffer_size_elements;
                num_elements_remaining -= actually_written;
                self.active_buffer_index += 1;
            } else {
                return result;
            }
        }
    }

    #[allow(unused)] // TODO(andreas): soon!
    #[inline]
    pub fn push(&mut self, element: T) -> Result<(), CpuWriteGpuReadError> {
        self.reserve(1)?;
        self.buffers[self.active_buffer_index].push(element)
    }

    /// Schedules copies of all previous writes to this `DataTextureSource` to a `GpuTexture`.
    ///
    /// The resulting `GpuTexture` is ready to be bound as a data texture in a shader.
    pub fn finish(
        self,
        texture_format: wgpu::TextureFormat,
        texture_label: impl Into<DebugLabel>,
    ) -> Result<GpuTexture, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        let total_num_elements: usize = self.buffers.iter().map(|b| b.num_written()).sum();

        let texture_desc = data_texture_desc(
            texture_label,
            texture_format,
            total_num_elements as u32,
            self.max_texture_width() as u32,
        );
        let data_texture = self
            .ctx
            .gpu_resources
            .textures
            .alloc(&self.ctx.device, &texture_desc);

        // Copy all buffers to the texture.
        let mut current_row = 0;
        let mut encoder = self.ctx.active_frame.before_view_builder_encoder.lock();

        for mut buffer in self.buffers.into_iter().take(self.active_buffer_index + 1) {
            // Buffer sizes were chosen such that they will always copy full rows!
            debug_assert!(buffer.capacity() % texture_desc.size.width as usize == 0);

            // The last buffer might need padding to fill a full row.
            let num_written = buffer.num_written();
            let num_elements_padding = buffer
                .num_written()
                .next_multiple_of(texture_desc.size.width as usize)
                - num_written;
            if num_elements_padding > 0 {
                buffer.fill_n(T::zeroed(), num_elements_padding)?;
            }
            let num_rows = buffer.num_written() / texture_desc.size.width as usize;

            buffer.copy_to_texture2d(
                encoder.get(),
                wgpu::ImageCopyTexture {
                    texture: &data_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: current_row,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    height: num_rows as u32,
                    ..texture_desc.size
                },
            )?;

            current_row += num_rows as u32;
        }

        Ok(data_texture)
    }
}

/// Texture size for storing a given amount of data.
///
/// For WebGL compatibility we sometimes have to use textures instead of buffers.
/// We call these textures "data textures".
/// This method determines the size of a data texture holding a given number of used texels.
/// Each texel is typically a single data entry (think `struct`).
///
/// `max_texture_dimension_2d` must be a power of two and is the maximum supported size of 2D textures.
///
/// For convenience, the returned texture size has a width such that its
/// row size in bytes is a multiple of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`.
/// This makes it a lot easier to copy data from a continuous buffer to the texture.
/// If we wouldn't do that, we'd need to do a copy for each row in some cases.
// TODO(andreas): everything should use `DataTextureSource` directly, then this function is no longer needed!
pub fn data_texture_size(
    format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> wgpu::Extent3d {
    debug_assert!(max_texture_dimension_2d.is_power_of_two());
    debug_assert!(!format.has_depth_aspect());
    debug_assert!(!format.has_stencil_aspect());
    debug_assert!(!format.is_compressed());

    let texel_size_in_bytes = format
        .block_copy_size(None)
        .expect("Depth/stencil formats are not supported as data textures");

    // Our data textures are usually accessed in a linear fashion, so ideally we'd be using a 1D texture.
    // However, 1D textures are very limited in size on many platforms, we have to use 2D textures instead.
    // 2D textures perform a lot better when their dimensions are powers of two, so we'll strictly stick to that even
    // when it seems to cause memory overhead.

    // We fill row by row. With the power-of-two requirement, this is the optimal strategy:
    // if there were a texture with less padding that uses half the width,
    // then we'd need to increase the height. We can't increase without doubling it, thus creating a texture
    // with the exact same mount of padding as before.

    let width = if num_texels_written < max_texture_dimension_2d {
        num_texels_written
            .next_power_of_two()
            // For too few number of written texels, or too small texels we might need to increase the size to stay
            // above a row **byte** size of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`.
            // Note that this implies that for very large texels, we need less wide textures to stay above this limit.
            // (width is in number of texels, but alignment cares about bytes!)
            .next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT / texel_size_in_bytes)
    } else {
        max_texture_dimension_2d
    };

    let height = num_texels_written.div_ceil(width);

    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// Texture descriptor for data storage.
///
/// See [`data_texture_size`]
// TODO(andreas): everything should use `DataTextureSource` directly, then this function is no longer needed!
pub fn data_texture_desc(
    label: impl Into<DebugLabel>,
    format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> wgpu_resources::TextureDesc {
    wgpu_resources::TextureDesc {
        label: label.into(),
        size: data_texture_size(format, num_texels_written, max_texture_dimension_2d),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    }
}

/// Pendent to [`data_texture_size`] for determining the element size (==texels on data texture)
/// need to be in a buffer that fills an entire data texture.
// TODO(andreas): everything should use `DataTextureSource` directly, then this function is no longer needed!
pub fn data_texture_source_buffer_element_count(
    texture_format: wgpu::TextureFormat,
    num_texels_written: u32,
    max_texture_dimension_2d: u32,
) -> usize {
    let data_texture_size =
        data_texture_size(texture_format, num_texels_written, max_texture_dimension_2d);
    let element_count = data_texture_size.width as usize * data_texture_size.height as usize;

    debug_assert!(element_count >= num_texels_written as usize);

    element_count
}
