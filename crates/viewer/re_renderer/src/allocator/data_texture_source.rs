use bytemuck::Pod;
use re_log::debug_assert;

use super::{CpuWriteGpuReadBuffer, CpuWriteGpuReadError};
use crate::wgpu_resources::{self, GpuTexture};
use crate::{DebugLabel, RenderContext};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum DataTextureSourceWriteError {
    #[error(
        "Reached maximum number of elements for a data texture of {max_num_elements} elements.
 Tried to add {num_elements_attempted_to_add} elements, but only added {num_elements_actually_added}."
    )]
    ReachedMaximumNumberOfElements {
        max_num_elements: usize,
        num_elements_attempted_to_add: usize,
        num_elements_actually_added: usize,
    },

    #[error(transparent)]
    CpuWriteGpuReadError(#[from] crate::CpuWriteGpuReadError),
}

/// Utility for writing data to a dynamically sized "data textures".
///
/// For WebGL compatibility we sometimes have to use textures instead of buffers.
/// We call these textures "data textures".
/// This construct allows to write data directly to gpu readable memory which
/// then upon finishing is automatically copied into an appropriately sized
/// texture which receives all data written to [`DataTextureSource`].
/// Each texel in the data texture represents a single element of the type `T`.
///
/// This is implemented by dynamically allocating cpu-write-gpu-read buffers from the
/// central [`super::CpuWriteGpuReadBelt`] and copying all of them to the texture in the end.
pub struct DataTextureSource<'a, T: Pod + Send + Sync> {
    ctx: &'a RenderContext, // TODO(andreas): Don't dependency inject, layers on top of this can do that.

    /// Buffers that need to be transferred to the data texture in the end.
    ///
    /// We have two options on how to deal with buffer allocation and filling:
    ///
    /// 1. fill the last buffer to its maximum capacity before starting writing to the next,
    ///    allow arbitrary amount of empty buffers
    ///    - Pro: makes the final gpu copies easy, we don't have to juggle weird offsets and several copies per buffer!
    ///    - Con: may need to spread writes over several buffers
    /// 2. create a new buffer whenever a write doesn't fully fit into the active buffer,
    ///    even if the last buffer has some remaining capacity, allow arbitrary amount of half-filled buffers
    ///    - Pro: All writes can go to a single buffer, which is simpler & faster.
    ///    - Con: We waste space and copying to the texture is harder
    ///
    /// We're going with option (1)!
    ///
    /// This means that there might be 'n' full buffers followed by a single active buffer, followed by 'm' empty buffers.
    buffers: Vec<CpuWriteGpuReadBuffer<T>>,

    /// Buffer in which data is currently written.
    ///
    /// Between all public calls:
    /// * all buffers before are full
    /// * all buffers after are empty (if any)
    /// * the buffer at this index either does not exist or has remaining capacity
    ///
    /// At the end of any operation that adds new elements, call
    /// `ensure_active_buffer_invariant_after_adding_elements` to ensure this invariant.
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

    /// Whether no elements have been written at all.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffers
            .first()
            .is_none_or(|first_buffer| first_buffer.is_empty())
    }

    /// The number of elements written so far.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffers
            .iter()
            .take(self.active_buffer_index + 1)
            .map(|b| b.num_written())
            .sum()
    }

    /// The number of elements that can be written without allocating more memory.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffers.iter().map(|b| b.capacity()).sum()
    }

    /// The number of elements that can be written without allocating more memory.
    #[inline]
    pub fn remaining_capacity(&self) -> usize {
        self.buffers
            .iter()
            .skip(self.active_buffer_index)
            .map(|b| b.remaining_capacity())
            .sum()
    }

    /// Ensure invariant that the active buffer has some remaining capacity or is the next buffer that needs to be allocated.
    ///
    /// Since elements were just added to the active buffer, this function assumes that the `active_buffer_index` points to a valid buffer.
    #[inline]
    fn ensure_active_buffer_invariant_after_adding_elements(&mut self) {
        debug_assert!(
            self.active_buffer_index < self.len(),
            "Active buffer index was expected to point at a valid buffer."
        );

        if self.buffers[self.active_buffer_index].remaining_capacity() == 0 {
            self.active_buffer_index += 1;
        }

        // Note that if we're *very* unlucky there might be a lot of unused buffers.
        // This happens only if there's quadratic growing `reserve` calls without any writes.
        debug_assert!(self.buffers.len() >= self.active_buffer_index);
        // If the active buffer exists, it must have remaining capacity.
        debug_assert!(
            self.buffers.len() == self.active_buffer_index
                || self.buffers[self.active_buffer_index].remaining_capacity() > 0
        );
        // The buffer before the active buffer must be full.
        debug_assert!(
            self.active_buffer_index == 0
                || self.buffers[self.active_buffer_index - 1].remaining_capacity() == 0
        );
    }

    /// Ensures that there's space internally for at least `num_elements` more elements.
    ///
    /// Returns the number of elements that are currently reserved.
    /// This value is *smaller* than the requested number of elements if the maximum number of
    /// elements that can be stored is reached, see [`max_num_elements_per_data_texture`].
    ///
    /// Creating new buffers is a relatively expensive operation, so it's best to
    /// reserve gratuitously and often. Ideally, you know exactly how many elements you're going to write and reserve
    /// accordingly at the start.
    ///
    /// If there's no more space, a new buffer is allocated such that:
    /// * have a total capacity for at least as many elements as requested, clamping total size to [`max_num_elements_per_data_texture`]
    /// * be at least double the size of the last buffer
    /// * keep it easy to copy to textures by always being a multiple of the maximum row size we use for data textures
    ///   - this massively simplifies the buffer->texture copy logic!
    pub fn reserve(&mut self, num_elements: usize) -> Result<usize, CpuWriteGpuReadError> {
        let remaining_capacity = self.remaining_capacity();
        if remaining_capacity >= num_elements {
            return Ok(remaining_capacity);
        }

        let max_texture_dimension_2d = self.ctx.device.limits().max_texture_dimension_2d;

        let last_buffer_size = self.buffers.last().map_or(0, |b| b.capacity());
        let new_buffer_size = (num_elements - remaining_capacity)
            .max(last_buffer_size * 2)
            .next_multiple_of(max_data_texture_width(max_texture_dimension_2d) as usize)
            .min(max_num_elements_per_data_texture(max_texture_dimension_2d) - self.capacity());

        if new_buffer_size > 0 {
            self.buffers
                .push(self.ctx.cpu_write_gpu_read_belt.lock().allocate(
                    &self.ctx.device,
                    &self.ctx.gpu_resources.buffers,
                    new_buffer_size,
                )?);
        }

        Ok(remaining_capacity + new_buffer_size)
    }

    fn error_on_clamped_write(
        &self,
        num_elements_attempted_to_add: usize,
        num_elements_actually_added: usize,
    ) -> Result<(), DataTextureSourceWriteError> {
        if num_elements_actually_added < num_elements_attempted_to_add {
            Err(
                DataTextureSourceWriteError::ReachedMaximumNumberOfElements {
                    max_num_elements: max_num_elements_per_data_texture(
                        self.ctx.device.limits().max_texture_dimension_2d,
                    ),
                    num_elements_attempted_to_add,
                    num_elements_actually_added,
                },
            )
        } else {
            Ok(())
        }
    }

    /// Pushes a slice of elements into the data texture.
    pub fn extend_from_slice(&mut self, elements: &[T]) -> Result<(), DataTextureSourceWriteError> {
        if elements.is_empty() {
            return Ok(());
        }

        re_tracing::profile_function_if!(10_000 < elements.len());

        let num_elements_available = self.reserve(elements.len())?;
        let total_elements_actually_added = num_elements_available.min(elements.len());

        let mut remaining_elements = &elements[..total_elements_actually_added];
        loop {
            let write_result =
                self.buffers[self.active_buffer_index].extend_from_slice(remaining_elements);

            // `extend_from_slice` is documented to write as many elements as possible, so we can just continue with the next buffer,
            // if we ran out of space!
            if let Err(CpuWriteGpuReadError::BufferFull {
                num_elements_actually_added,
                ..
            }) = write_result
            {
                remaining_elements = &remaining_elements[num_elements_actually_added..];
                self.active_buffer_index += 1; // Due to the prior `reserve` call we know that there's more buffers!
            } else {
                self.ensure_active_buffer_invariant_after_adding_elements();
                write_result?;
                return self.error_on_clamped_write(elements.len(), total_elements_actually_added);
            }
        }
    }

    /// Fills the data texture with n instances of an element.
    pub fn add_n(
        &mut self,
        element: T,
        num_elements: usize,
    ) -> Result<(), DataTextureSourceWriteError> {
        if num_elements == 0 {
            return Ok(());
        }

        re_tracing::profile_function_if!(10_000 < num_elements);

        let num_elements_available = self.reserve(num_elements)?;
        let total_elements_actually_added = num_elements_available.min(num_elements);

        let mut num_elements_remaining = total_elements_actually_added;
        loop {
            let write_result =
                self.buffers[self.active_buffer_index].add_n(element, num_elements_remaining);

            // `fill_n` is documented to write as many elements as possible, so we can just continue with the next buffer,
            // if we ran out of space!
            if let Err(CpuWriteGpuReadError::BufferFull {
                num_elements_actually_added,
                ..
            }) = write_result
            {
                num_elements_remaining -= num_elements_actually_added;
                self.active_buffer_index += 1; // Due to the prior `reserve` call we know that there's more buffers!
            } else {
                self.ensure_active_buffer_invariant_after_adding_elements();
                write_result?;
                return self.error_on_clamped_write(num_elements, total_elements_actually_added);
            }
        }
    }

    #[inline]
    pub fn push(&mut self, element: T) -> Result<(), DataTextureSourceWriteError> {
        if self.reserve(1)? < 1 {
            return self.error_on_clamped_write(1, 0);
        }

        self.buffers[self.active_buffer_index].push(element)?;
        self.ensure_active_buffer_invariant_after_adding_elements();

        Ok(())
    }

    fn data_texture_size(&self, max_texture_dimension_2d: u32) -> wgpu::Extent3d {
        let texel_size_in_bytes = std::mem::size_of::<T>() as u32;
        let num_texels = self.len();

        debug_assert!(num_texels <= max_num_elements_per_data_texture(max_texture_dimension_2d));

        // Our data textures are usually accessed in a linear fashion, so ideally we'd be using a 1D texture.
        // However, 1D textures are very limited in size on many platforms, we have to use 2D textures instead.
        // 2D textures perform a lot better when their dimensions are powers of two, so we'll strictly stick to that even
        // when it seems to cause memory overhead.

        // We fill row by row. With the power-of-two requirement, this is the optimal strategy:
        // if there were a texture with less padding that uses half the width,
        // then we'd need to increase the height. We can't increase without doubling it, thus creating a texture
        // with the exact same mount of padding as before.
        let max_data_texture_width = max_data_texture_width(max_texture_dimension_2d);
        let width = if num_texels < max_data_texture_width as usize {
            num_texels
                .next_power_of_two()
                // For too few number of written texels, or too small texels we might need to increase the size to stay
                // above a row **byte** size of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`.
                // Note that this implies that for very large texels, we need less wide textures to stay above this limit.
                // (width is in number of texels, but alignment cares about bytes!)
                .next_multiple_of(
                    (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT / texel_size_in_bytes) as usize,
                ) as u32
        } else {
            max_data_texture_width
        };

        let height = num_texels.div_ceil(width as usize);
        debug_assert!(height <= max_texture_dimension_2d as usize); // Texel count should have been clamped accordingly already!

        wgpu::Extent3d {
            width,
            height: height as u32,
            depth_or_array_layers: 1,
        }
    }

    /// Schedules copies of all previous writes to this `DataTextureSource` to a `GpuTexture`.
    ///
    /// The format has to be uncompressed, not a depth/stencil format and have the exact same block size of the size of type `T`.
    /// The resulting `GpuTexture` is ready to be bound as a data texture in a shader.
    pub fn finish(
        self,
        texture_format: wgpu::TextureFormat,
        texture_label: impl Into<DebugLabel>,
    ) -> Result<GpuTexture, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        debug_assert!(!texture_format.has_depth_aspect());
        debug_assert!(!texture_format.has_stencil_aspect());
        debug_assert!(!texture_format.is_compressed());
        re_log::debug_assert_eq!(
            texture_format
                .block_copy_size(None)
                .expect("Depth/stencil formats are not supported as data textures"),
            std::mem::size_of::<T>() as u32,
        );

        let texture_size =
            self.data_texture_size(self.ctx.device.limits().max_texture_dimension_2d);
        let data_texture = self.ctx.gpu_resources.textures.alloc(
            &self.ctx.device,
            &wgpu_resources::TextureDesc {
                label: texture_label.into(),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: texture_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );
        let texture_width = texture_size.width as usize;

        // Copy all buffers to the texture.
        let mut current_row = 0;
        let mut encoder = self.ctx.active_frame.before_view_builder_encoder.lock();

        for mut buffer in self.buffers.into_iter().take(self.active_buffer_index + 1) {
            // Buffer sizes were chosen such that they will always copy full rows!
            debug_assert!(buffer.capacity() % texture_width == 0);

            // The last buffer might need padding to fill a full row.
            let num_written = buffer.num_written();
            let num_elements_padding =
                buffer.num_written().next_multiple_of(texture_width) - num_written;
            buffer.add_n(T::zeroed(), num_elements_padding)?;

            let num_rows = buffer.num_written() / texture_width;

            buffer.copy_to_texture2d(
                encoder.get(),
                wgpu::TexelCopyTextureInfo {
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
                    ..texture_size
                },
            )?;

            current_row += num_rows as u32;
        }

        Ok(data_texture)
    }
}

/// Maximum width for data textures.
#[inline]
fn max_data_texture_width(max_texture_dimension_2d: u32) -> u32 {
    // We limit the data texture width to 16384 or whatever smaller value is supported but the device.
    //
    // If we make buffers always a multiple of this width, we can do all buffer copies in a single copy!
    //
    // But wait! If we're using this as the minimum buffer size, isn't that too big?
    // 16384 * float4 (worst case) = 256KiB.
    // Keep in mind that weaker hardware will have 8192 max width.
    // Also note, that many of our textures use 4 & 8 byte formats.
    // So while this is still a considerable amount of memory when used for very small data textures
    // it's not as bad as it seems.
    // Given how much it simplifies to keep buffers a multiple of the texture width,
    // this seems to be a reasonable trade-off.
    (max_texture_dimension_2d).min(16384)
}

/// Maximum number of elements that can be written to a single data texture.
#[inline]
fn max_num_elements_per_data_texture(max_texture_dimension_2d: u32) -> usize {
    let max_width = max_data_texture_width(max_texture_dimension_2d) as usize;
    let max_height = max_texture_dimension_2d as usize;
    max_width * max_height
}
