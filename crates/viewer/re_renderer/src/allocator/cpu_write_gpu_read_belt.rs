use std::sync::mpsc;

use re_log::debug_assert;

use crate::texture_info::Texture2DBufferInfo;
use crate::wgpu_resources::{BufferDesc, GpuBuffer, GpuBufferPool, GpuTexture};

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum CpuWriteGpuReadError {
    #[error("Attempting to allocate an empty buffer.")]
    ZeroSizeBufferAllocation,

    #[error(
        "Buffer is full, can't append more data! Buffer has a capacity for {buffer_capacity_elements} elements.
 Tried to add {num_elements_attempted_to_add} elements, but only added {num_elements_actually_added}."
    )]
    BufferFull {
        buffer_capacity_elements: usize,
        num_elements_attempted_to_add: usize,
        num_elements_actually_added: usize,
    },

    #[error(
        "Target buffer has a size of {target_buffer_size}, can't write {copy_size} bytes with an offset of {destination_offset}!"
    )]
    TargetBufferTooSmall {
        target_buffer_size: u64,
        copy_size: u64,
        destination_offset: u64,
    },

    #[error(
        "Target texture doesn't fit the size of the written data to this buffer! Texture target buffer should be at most {max_copy_size} bytes, but the to be written data was {written_data_size} bytes."
    )]
    TargetTextureBufferSizeMismatch {
        max_copy_size: u64,
        written_data_size: usize,
    },
}

/// A sub-allocated staging buffer that can be written to.
///
/// Behaves a bit like a fixed sized `Vec` in that far it keeps track of how many elements were written to it.
///
/// We do *not* allow reading from this buffer as it is typically write-combined memory.
/// Reading would work, but it can be *very* slow.
/// For details on why, see
/// [Write combining is not your friend, by Fabian Giesen](https://fgiesen.wordpress.com/2013/01/29/write-combining-is-not-your-friend/)
/// Note that the "vec like behavior" further encourages
/// * not leaving holes
/// * keeping writes sequential
///
/// Must be dropped before calling [`CpuWriteGpuReadBelt::before_queue_submit`] (typically the end of a frame).
/// If this buffer is not dropped before calling [`CpuWriteGpuReadBelt::before_queue_submit`], a validation error will occur.
pub struct CpuWriteGpuReadBuffer<T: bytemuck::Pod + Send + Sync> {
    /// Write view into the relevant buffer portion.
    write_view: wgpu::BufferViewMut,

    /// Range in T elements in `write_view` that haven't been written yet.
    unwritten_element_range: std::ops::Range<usize>,

    chunk_buffer: GpuBuffer,
    byte_offset_in_chunk_buffer: wgpu::BufferAddress,

    /// Marker for the type whose alignment and size requirements are honored by `write_view`.
    _type: std::marker::PhantomData<T>,
}

impl<T> CpuWriteGpuReadBuffer<T>
where
    T: bytemuck::Pod + Send + Sync,
{
    /// Memory as slice.
    ///
    /// Note that we can't rely on any alignment guarantees here!
    /// We could offset the mapped CPU-sided memory, but then the GPU offset won't be aligned anymore.
    /// There's no way we can meet conflicting alignment requirements, so we need to work with unaligned bytes instead.
    /// See [this comment on this wgpu issue](https://github.com/gfx-rs/wgpu/issues/3508#issuecomment-1485044324) about what we tried before.
    ///
    /// Once wgpu has some alignment guarantees, we might be able to use this here to allow faster copies!
    /// (copies of larger blocks are likely less affected as `memcpy` typically does dynamic check/dispatching for SIMD based copies)
    ///
    /// Do *not* make this public as we need to guarantee that the memory is *never* read from!
    #[inline(always)]
    fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        // TODO(andreas): Is this access slow given that it internally goes through a trait interface? Should we keep the pointer around?
        &mut self.write_view[self.unwritten_element_range.start * std::mem::size_of::<T>()
            ..self.unwritten_element_range.end * std::mem::size_of::<T>()]
    }

    /// Pushes a slice of elements into the buffer.
    ///
    /// If the buffer is not big enough, only the first `self.remaining_capacity()` elements are pushed before returning an error.
    #[inline]
    pub fn extend_from_slice(&mut self, elements: &[T]) -> Result<(), CpuWriteGpuReadError> {
        if elements.is_empty() {
            return Ok(());
        }

        re_tracing::profile_function_if!(10_000 < elements.len());

        let remaining_capacity = self.remaining_capacity();
        let (result, elements) = if elements.len() > remaining_capacity {
            (
                Err(CpuWriteGpuReadError::BufferFull {
                    buffer_capacity_elements: self.capacity(),
                    num_elements_attempted_to_add: elements.len(),
                    num_elements_actually_added: remaining_capacity,
                }),
                &elements[..remaining_capacity],
            )
        } else {
            (Ok(()), elements)
        };

        let bytes = bytemuck::cast_slice(elements);
        self.as_mut_byte_slice()[..bytes.len()].copy_from_slice(bytes);
        self.unwritten_element_range.start += elements.len();

        result
    }

    /// Pushes several elements into the buffer.
    ///
    /// If the buffer is not big enough, only the first [`CpuWriteGpuReadBuffer::remaining_capacity`] elements are pushed before returning an error.
    /// Otherwise, returns the number of elements pushed for convenience.
    #[inline]
    pub fn extend(
        &mut self,
        mut elements: impl ExactSizeIterator<Item = T>,
    ) -> Result<usize, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        // TODO(emilk): optimize the extend function.
        // Right now it is 3-4x faster to collect to a vec first, which is crazy.
        if true {
            let vec: Vec<T> = elements.collect();

            re_log::debug_assert_eq!(
                vec.as_ptr() as usize % std::mem::align_of::<T>(),
                0,
                "Vec::collect collects into unaligned memory! Is this a bug in the allocator?"
            );

            self.extend_from_slice(vec.as_slice())?;
            Ok(vec.len())
        } else {
            let num_written_before = self.num_written();

            while let Some(element) = elements.next() {
                if self.unwritten_element_range.start >= self.unwritten_element_range.end {
                    let num_elements_actually_added = self.num_written() - num_written_before;

                    return Err(CpuWriteGpuReadError::BufferFull {
                        buffer_capacity_elements: self.capacity(),
                        num_elements_attempted_to_add: num_elements_actually_added
                            + elements.count()
                            + 1,
                        num_elements_actually_added,
                    });
                }

                self.as_mut_byte_slice()[..std::mem::size_of::<T>()]
                    .copy_from_slice(bytemuck::bytes_of(&element));
                self.unwritten_element_range.start += 1;
            }

            Ok(self.num_written() - num_written_before)
        }
    }

    /// Fills the buffer with n instances of an element.
    ///
    /// If the buffer is not big enough, only the first `self.remaining_capacity()` elements are pushed before returning an error.
    pub fn add_n(&mut self, element: T, num_elements: usize) -> Result<(), CpuWriteGpuReadError> {
        if num_elements == 0 {
            return Ok(());
        }

        re_tracing::profile_function_if!(10_000 < num_elements);

        let remaining_capacity = self.remaining_capacity();
        let (result, num_elements) = if num_elements > remaining_capacity {
            (
                Err(CpuWriteGpuReadError::BufferFull {
                    buffer_capacity_elements: self.capacity(),
                    num_elements_attempted_to_add: num_elements,
                    num_elements_actually_added: remaining_capacity,
                }),
                remaining_capacity,
            )
        } else {
            (Ok(()), num_elements)
        };

        let mut offset = 0;
        let buffer_bytes = self.as_mut_byte_slice();
        let element_bytes = bytemuck::bytes_of(&element);

        for _ in 0..num_elements {
            let end = offset + std::mem::size_of::<T>();
            buffer_bytes[offset..end].copy_from_slice(element_bytes);
            offset = end;
        }
        self.unwritten_element_range.start += num_elements;

        result
    }

    /// Pushes a single element into the buffer and advances the write pointer.
    ///
    /// Returns an error if the data no longer fits into the buffer.
    #[inline]
    pub fn push(&mut self, element: T) -> Result<(), CpuWriteGpuReadError> {
        if self.remaining_capacity() == 0 {
            return Err(CpuWriteGpuReadError::BufferFull {
                buffer_capacity_elements: self.capacity(),
                num_elements_attempted_to_add: 1,
                num_elements_actually_added: 0,
            });
        }

        self.as_mut_byte_slice()[..std::mem::size_of::<T>()]
            .copy_from_slice(bytemuck::bytes_of(&element));
        self.unwritten_element_range.start += 1;

        Ok(())
    }

    /// True if no elements have been pushed into the buffer so far.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.unwritten_element_range.start == 0
    }

    /// The number of elements pushed into the buffer so far.
    #[inline]
    pub fn num_written(&self) -> usize {
        self.unwritten_element_range.start
    }

    /// The number of elements that can still be pushed into the buffer.
    #[inline]
    pub fn remaining_capacity(&self) -> usize {
        self.unwritten_element_range.end - self.unwritten_element_range.start
    }

    /// Total number of elements that the buffer can hold.
    pub fn capacity(&self) -> usize {
        self.unwritten_element_range.end
    }

    /// Copies all so far written data to the first layer of a 2D texture.
    ///
    /// Assumes that the buffer consists of as-tightly-packed-as-possible rows of data.
    /// (taking into account required padding as specified by [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`])
    ///
    /// Fails if the buffer size is not sufficient to fill the entire texture.
    pub fn copy_to_texture2d_entire_first_layer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &GpuTexture,
    ) -> Result<(), CpuWriteGpuReadError> {
        self.copy_to_texture2d(
            encoder,
            wgpu::TexelCopyTextureInfo {
                texture: &destination.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            destination.texture.size(),
        )
    }

    /// Copies all so far written data to a rectangle on a single 2D texture layer.
    ///
    /// Assumes that the buffer consists of as-tightly-packed-as-possible rows of data.
    /// (taking into account required padding as specified by [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`])
    ///
    /// Implementation note:
    /// Does 2D-only entirely for convenience as it greatly simplifies the input parameters.
    pub fn copy_to_texture2d(
        self,
        encoder: &mut wgpu::CommandEncoder,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) -> Result<(), CpuWriteGpuReadError> {
        let buffer_info = Texture2DBufferInfo::new(destination.texture.format(), copy_size);

        // Validate that we stay within the written part of the slice (wgpu can't fully know our intention here, so we have to check).
        // This is a bit of a leaky check since we haven't looked at copy_size which may limit the amount of memory we need.
        if (buffer_info.buffer_size_padded as usize) < self.num_written() * std::mem::size_of::<T>()
        {
            return Err(CpuWriteGpuReadError::TargetTextureBufferSizeMismatch {
                max_copy_size: buffer_info.buffer_size_padded,
                written_data_size: self.num_written() * std::mem::size_of::<T>(),
            });
        }

        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &self.chunk_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: self.byte_offset_in_chunk_buffer,
                    bytes_per_row: Some(buffer_info.bytes_per_row_padded),
                    rows_per_image: None,
                },
            },
            destination,
            copy_size,
        );

        Ok(())
    }

    /// Copies the entire buffer to another buffer and drops it.
    pub fn copy_to_buffer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &GpuBuffer,
        destination_offset: wgpu::BufferAddress,
    ) -> Result<(), CpuWriteGpuReadError> {
        let copy_size = (std::mem::size_of::<T>() * self.unwritten_element_range.start) as u64;

        // Wgpu does validation as well, but we want to be able to track this error right away.
        if copy_size > destination_offset + destination.size() {
            return Err(CpuWriteGpuReadError::TargetBufferTooSmall {
                target_buffer_size: destination.size(),
                copy_size,
                destination_offset,
            });
        }

        encoder.copy_buffer_to_buffer(
            &self.chunk_buffer,
            self.byte_offset_in_chunk_buffer,
            destination,
            destination_offset,
            copy_size,
        );

        Ok(())
    }
}

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBuffer,

    /// Starting at this offset the buffer is unused.
    unused_offset: wgpu::BufferAddress,
}

impl Chunk {
    fn remaining_capacity(&self) -> u64 {
        self.buffer.size() - self.unused_offset
    }

    /// Caller needs to make sure that there is enough space.
    fn allocate<T: bytemuck::Pod + Send + Sync>(
        &mut self,
        num_elements: usize,
        size_in_bytes: u64,
    ) -> CpuWriteGpuReadBuffer<T> {
        debug_assert!(num_elements * std::mem::size_of::<T>() <= size_in_bytes as usize);

        let byte_offset_in_chunk_buffer = self.unused_offset;
        let end_offset = byte_offset_in_chunk_buffer + size_in_bytes;

        debug_assert!(
            byte_offset_in_chunk_buffer.is_multiple_of(CpuWriteGpuReadBelt::MIN_OFFSET_ALIGNMENT)
        );
        debug_assert!(end_offset <= self.buffer.size());

        let buffer_slice = self.buffer.slice(byte_offset_in_chunk_buffer..end_offset);
        let write_view = buffer_slice.get_mapped_range_mut();
        self.unused_offset = end_offset;

        CpuWriteGpuReadBuffer {
            chunk_buffer: self.buffer.clone(),
            byte_offset_in_chunk_buffer,
            write_view,
            unwritten_element_range: 0..num_elements,
            _type: std::marker::PhantomData,
        }
    }
}

/// Efficiently performs many buffer writes by sharing and reusing temporary buffers.
///
/// Internally it uses a ring-buffer of staging buffers that are sub-allocated.
///
/// Based on to [`wgpu::util::StagingBelt`](https://github.com/gfx-rs/wgpu/blob/a420e453c3d9c93dfb1a8526bf11c000d895c916/wgpu/src/util/belt.rs)
/// However, there are some important differences:
/// * can create buffers without yet knowing the target copy location
/// * lifetime of returned buffers is independent of the [`CpuWriteGpuReadBelt`] (allows working with several in parallel!)
/// * use of `re_renderer`'s resource pool
/// * handles alignment in a defined manner
///   (see this as of writing open wgpu issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508))
pub struct CpuWriteGpuReadBelt {
    /// Minimum size for new buffers.
    chunk_size: u64,

    /// Chunks which are CPU write at the moment.
    active_chunks: Vec<Chunk>,

    /// Chunks which are GPU read at the moment.
    ///
    /// I.e. they have scheduled transfers already; they are unmapped and one or more
    /// command encoder has one or more `copy_buffer_to_buffer` commands with them
    /// as source.
    closed_chunks: Vec<Chunk>,

    /// Chunks that are back from the GPU and ready to be mapped for write and put
    /// into `active_chunks`.
    free_chunks: Vec<Chunk>,

    /// When closed chunks are mapped again, the map callback sends them here.
    ///
    /// Note that we shouldn't use `SyncSender` since this can block the `Sender` if a buffer is full,
    /// which means that in a single threaded situation (Web!) we might deadlock.
    sender: mpsc::Sender<Chunk>,

    /// Free chunks are received here to be put on `self.free_chunks`.
    receiver: mpsc::Receiver<Chunk>,
}

impl CpuWriteGpuReadBelt {
    /// All allocations of this allocator will be aligned to at least this size.
    ///
    /// Requiring a minimum alignment means we need to pad less often.
    /// Also, it has the potential of making memcpy operations faster.
    ///
    /// Needs to be larger or equal than [`wgpu::MAP_ALIGNMENT`], [`wgpu::COPY_BUFFER_ALIGNMENT`]
    /// and the largest possible texel block footprint (since offsets for texture copies require this)
    ///
    /// For alignment requirements in `WebGPU` in general, refer to
    /// [the specification on alignment-class limitations](https://www.w3.org/TR/webgpu/#limit-class-alignment)
    ///
    /// Note that this does NOT mean that the CPU memory has *any* alignment.
    /// See this issue about [lack of CPU memory alignment](https://github.com/gfx-rs/wgpu/issues/3508) in wgpu/WebGPU.
    const MIN_OFFSET_ALIGNMENT: u64 = 16;

    /// Create a cpu-write & gpu-read staging belt.
    ///
    /// The `chunk_size` is the unit of internal buffer allocation; writes will be
    /// sub-allocated within each chunk. Therefore, for optimal use of memory, the
    /// chunk size should be:
    ///
    /// * larger than the largest single [`CpuWriteGpuReadBelt::allocate`] operation;
    /// * 1-4 times less than the total amount of data uploaded per submission
    ///   (per [`CpuWriteGpuReadBelt::before_queue_submit()`]); and
    /// * bigger is better, within these bounds.
    ///
    /// TODO(andreas): Adaptive chunk sizes
    /// TODO(andreas): Shrinking after usage spikes?
    pub fn new(chunk_size: wgpu::BufferSize) -> Self {
        static_assertions::const_assert!(
            wgpu::MAP_ALIGNMENT <= CpuWriteGpuReadBelt::MIN_OFFSET_ALIGNMENT
        );
        static_assertions::const_assert!(
            wgpu::COPY_BUFFER_ALIGNMENT <= CpuWriteGpuReadBelt::MIN_OFFSET_ALIGNMENT
        );
        // Largest uncompressed texture format (btw. many compressed texture format have the same block size!)
        debug_assert!(
            wgpu::TextureFormat::Rgba32Uint
                .block_copy_size(None)
                .unwrap() as u64
                <= Self::MIN_OFFSET_ALIGNMENT
        );

        // we must use an unbounded channel to avoid blocking on web
        #[expect(clippy::disallowed_methods)]
        let (sender, receiver) = mpsc::channel();
        Self {
            chunk_size: wgpu::util::align_to(chunk_size.get(), Self::MIN_OFFSET_ALIGNMENT),
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Allocates a cpu writable buffer for `num_elements` instances of type `T`.
    ///
    /// The buffer will be aligned to T's alignment, but no less than [`Self::MIN_OFFSET_ALIGNMENT`].
    pub fn allocate<T: bytemuck::Pod + Send + Sync>(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &GpuBufferPool,
        num_elements: usize,
    ) -> Result<CpuWriteGpuReadBuffer<T>, CpuWriteGpuReadError> {
        if num_elements == 0 {
            return Err(CpuWriteGpuReadError::ZeroSizeBufferAllocation);
        }

        re_tracing::profile_function!();

        debug_assert!(num_elements > 0, "Cannot allocate zero-sized buffer");

        // Potentially overestimate size with Self::MIN_ALIGNMENT, see Self::MIN_ALIGNMENT doc string.
        let size = wgpu::util::align_to(
            (std::mem::size_of::<T>() * num_elements) as wgpu::BufferAddress,
            Self::MIN_OFFSET_ALIGNMENT,
        );

        // Try to find space in any of the active chunks first.
        let mut chunk = if let Some(index) = self
            .active_chunks
            .iter_mut()
            .position(|chunk| chunk.remaining_capacity() >= size)
        {
            self.active_chunks.swap_remove(index)
        } else {
            self.receive_chunks(); // ensure self.free_chunks is up to date

            // Use a free chunk if possible, fall back to creating a new one if necessary.
            if let Some(index) = self
                .free_chunks
                .iter()
                .position(|chunk| chunk.remaining_capacity() >= size)
            {
                self.free_chunks.swap_remove(index)
            } else {
                // Allocation might be bigger than a chunk!
                let buffer_size =
                    wgpu::util::align_to(self.chunk_size.max(size), Self::MIN_OFFSET_ALIGNMENT);
                re_log::trace!(
                    "Allocating new CpuWriteGpuReadBelt chunk of size {:.1} MiB",
                    buffer_size as f32 / (1024.0 * 1024.0)
                );
                let buffer = buffer_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "CpuWriteGpuReadBelt chunk buffer".into(),
                        size: buffer_size,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: true,
                    },
                );

                Chunk {
                    buffer,
                    unused_offset: 0,
                }
            }
        };

        let cpu_buffer_view = chunk.allocate(num_elements, size);
        self.active_chunks.push(chunk);
        Ok(cpu_buffer_view)
    }

    /// Prepare currently mapped buffers for use in a submission.
    ///
    /// All existing [`CpuWriteGpuReadBuffer`] MUST be dropped before calling this function.
    /// Any not dropped [`CpuWriteGpuReadBuffer`] will cause a validation error.
    ///
    /// This must be called BEFORE the command encoder(s) used in any [`CpuWriteGpuReadBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed (cannot be used for
    /// further writes) until after [`CpuWriteGpuReadBelt::after_queue_submit`] is called *and* the GPU is done
    /// copying the data from them.
    pub fn before_queue_submit(&mut self) {
        re_tracing::profile_function!();

        // This would be a great usecase for persistent memory mapping, i.e. mapping without the need to unmap
        // https://github.com/gfx-rs/wgpu/issues/1468
        // However, WebGPU does not support this!

        // We're done with writing to this chunk and are ready to have the GPU read it!
        //
        // This part has to happen before submit, otherwise we get a validation error that
        // the buffers are still mapped and can't be read by the gpu.
        for chunk in self.active_chunks.drain(..) {
            chunk.buffer.unmap();
            self.closed_chunks.push(chunk);
        }
    }

    /// Recall all of the closed buffers back to be reused.
    ///
    /// This must only be called after the command encoder(s) used in [`CpuWriteGpuReadBuffer`]
    /// copy operations are submitted. Additional calls are harmless.
    /// Not calling this as soon as possible may result in increased buffer memory usage.
    ///
    /// Implementation note:
    /// We can't use [`wgpu::CommandEncoder::map_buffer_on_submit`] here because for that we'd need to know which
    /// command encoder is the last one scheduling any cpu->gpu copy operations.
    /// Note that if chunks were fully tied to a single encoder, we could call [`wgpu::CommandEncoder::map_buffer_on_submit`]
    /// once we know a chunk has all its cpu->gpu copy operations scheduled on that very encoder.
    pub fn after_queue_submit(&mut self) {
        re_tracing::profile_function!();
        self.receive_chunks();

        let sender = &self.sender;
        for chunk in self.closed_chunks.drain(..) {
            let sender = sender.clone();
            chunk
                .buffer
                .clone()
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |_| {
                    sender.send(chunk).ok();
                });
        }
    }

    /// Move all chunks that the GPU is done with (and are now mapped again)
    /// from `self.receiver` to `self.free_chunks`.
    fn receive_chunks(&mut self) {
        while let Ok(mut chunk) = self.receiver.try_recv() {
            chunk.unused_offset = 0;
            self.free_chunks.push(chunk);
        }
    }
}

impl std::fmt::Debug for CpuWriteGpuReadBelt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuWriteGpuReadBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("closed_chunks", &self.closed_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .finish_non_exhaustive()
    }
}
