use std::{num::NonZeroU32, ops::DerefMut, sync::mpsc};

use crate::wgpu_resources::{BufferDesc, GpuBuffer, GpuBufferPool};

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
pub struct CpuWriteGpuReadBuffer<T: bytemuck::Pod + 'static> {
    /// Write view into the relevant buffer portion.
    ///
    /// UNSAFE: The lifetime is transmuted to be `'static`.
    /// In actuality it is tied to the lifetime of [`chunk_buffer`](#structfield.chunk_buffer)!
    write_view: wgpu::BufferViewMut<'static>,

    /// Range in T elements in write_view that haven't been written yet.
    unwritten_element_range: std::ops::Range<usize>,

    chunk_buffer: GpuBuffer,
    byte_offset_in_chunk_buffer: wgpu::BufferAddress,

    /// Marker for the type whose alignment and size requirements are honored by `write_view`.
    _type: std::marker::PhantomData<T>,
}

impl<T> CpuWriteGpuReadBuffer<T>
where
    T: bytemuck::Pod + 'static,
{
    /// Memory as slice of T.
    ///
    /// Do *not* make this public as we need to guarantee that the memory is *never* read from!
    #[inline(always)]
    fn as_slice(&mut self) -> &mut [T] {
        &mut bytemuck::cast_slice_mut(&mut self.write_view)[self.unwritten_element_range.clone()]
    }

    /// Pushes a slice of elements into the buffer.
    ///
    /// Panics if the data no longer fits into the buffer.
    #[inline]
    pub fn extend_from_slice(&mut self, elements: &[T]) {
        self.as_slice().copy_from_slice(elements);
        self.unwritten_element_range.start += elements.len();
    }

    /// Pushes several elements into the buffer.
    ///
    /// Panics if the data no longer fits into the buffer.
    #[inline]
    pub fn extend(&mut self, elements: impl Iterator<Item = T>) {
        let mut num_elements = 0;
        for (target, source) in self.as_slice().iter_mut().zip(elements) {
            *target = source;
            num_elements += 1;
        }
        self.unwritten_element_range.start += num_elements;
    }

    /// Pushes a single element into the buffer and advances the write pointer.
    ///
    /// Panics if the data no longer fits into the buffer.
    #[inline]
    pub fn push(&mut self, element: T) {
        self.as_slice()[0] = element;
        self.unwritten_element_range.start += 1;
    }

    /// The number of elements pushed into the buffer so far.
    #[inline]
    pub fn num_written(&self) -> usize {
        self.unwritten_element_range.start
    }

    /// Copies the entire buffer to a texture and drops it.
    pub fn copy_to_texture(
        self,
        encoder: &mut wgpu::CommandEncoder,
        destination: wgpu::ImageCopyTexture<'_>,
        bytes_per_row: Option<NonZeroU32>,
        rows_per_image: Option<NonZeroU32>,
        copy_size: wgpu::Extent3d,
    ) {
        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &self.chunk_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: self.byte_offset_in_chunk_buffer,
                    bytes_per_row,
                    rows_per_image,
                },
            },
            destination,
            copy_size,
        );
    }

    /// Copies the entire buffer to another buffer and drops it.
    pub fn copy_to_buffer(
        mut self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &GpuBuffer,
        destination_offset: wgpu::BufferAddress,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.chunk_buffer,
            self.byte_offset_in_chunk_buffer,
            destination,
            destination_offset,
            self.write_view.deref_mut().len() as u64,
        );
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

    fn required_padding(write_view: &mut wgpu::BufferViewMut<'_>, alignment: u64) -> u64 {
        // Use deref_mut explicitly because wgpu warns otherwise that read access is slow.
        let ptr = write_view.deref_mut().as_ptr() as u64;
        wgpu::util::align_to(ptr, alignment) - ptr
    }

    /// Caller needs to make sure that there is enough space plus potential padding.
    fn allocate_aligned<T: bytemuck::Pod>(
        &mut self,
        num_elements: usize,
        size_in_bytes: u64,
        alignment: u64,
    ) -> CpuWriteGpuReadBuffer<T> {
        debug_assert!(num_elements * std::mem::size_of::<T>() <= size_in_bytes as usize);

        // Optimistic first mapping attempt.
        let mut start_offset = self.unused_offset;
        let mut end_offset = start_offset + size_in_bytes;
        let mut buffer_slice = self.buffer.slice(start_offset..end_offset);
        let mut write_view = buffer_slice.get_mapped_range_mut();

        // Check if it fulfills the requested alignment.
        let required_padding = Self::required_padding(&mut write_view, alignment);
        if required_padding != 0 {
            // Undo mapping and try again with padding!
            re_log::trace!(
                "CpuWriteGpuReadBuffer::allocate alignment requirement not fulfilled. Need to add {required_padding} for alignment of {alignment}"
            );

            drop(write_view);

            start_offset = self.unused_offset + required_padding;
            end_offset = start_offset + size_in_bytes;
            buffer_slice = self.buffer.slice(start_offset..end_offset);
            write_view = buffer_slice.get_mapped_range_mut();

            let required_padding = Self::required_padding(&mut write_view, alignment);
            debug_assert_eq!(required_padding, 0);
        }

        debug_assert!(end_offset <= self.buffer.size());
        self.unused_offset = end_offset;

        #[allow(unsafe_code)]
        // SAFETY:
        // write_view has a lifetime dependency on the chunk's buffer - internally it holds a pointer to it!
        //
        // To ensure that the buffer is still around, we put the ref counted buffer handle into the struct with it.
        // Additionally, the buffer pool needs to ensure:
        // * it can't drop buffers if there's still users
        //      -> We assert on on that
        // * buffers are never moved in memory
        //      -> buffers are always owned by the pool and are always Arc.
        //          This means it not allowed to move the buffer out.
        //          (We could make them Pin<Arc<>> but this complicates things inside the BufferPool)
        let write_view = unsafe {
            std::mem::transmute::<wgpu::BufferViewMut<'_>, wgpu::BufferViewMut<'static>>(write_view)
        };

        CpuWriteGpuReadBuffer {
            chunk_buffer: self.buffer.clone(),
            byte_offset_in_chunk_buffer: start_offset,
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
///  (see this as of writing open wgpu issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508))
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
    /// Note that we shouldn't use SyncSender since this can block the Sender if a buffer is full,
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
    /// Align to 4xf32. Should be enough for most usecases!
    /// Needs to be larger or equal than [`wgpu::MAP_ALIGNMENT`].
    /// For alignment requirements in `WebGPU` in general, refer to
    /// [the specification on alignment-class limitations](https://www.w3.org/TR/webgpu/#limit-class-alignment)
    pub const MIN_ALIGNMENT: u64 = 16;

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
        static_assertions::const_assert!(wgpu::MAP_ALIGNMENT <= CpuWriteGpuReadBelt::MIN_ALIGNMENT);

        let (sender, receiver) = mpsc::channel();
        CpuWriteGpuReadBelt {
            chunk_size: wgpu::util::align_to(chunk_size.get(), Self::MIN_ALIGNMENT),
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Allocates a cpu writable buffer for `num_elements` instances of type `T`.
    ///
    /// Handles alignment requirements automatically, allowing arbitrarily aligned types without issues.
    pub fn allocate<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &mut GpuBufferPool,
        num_elements: usize,
    ) -> CpuWriteGpuReadBuffer<T> {
        // Potentially overestimate alignment with Self::MIN_ALIGNMENT, see Self::MIN_ALIGNMENT doc string.
        let alignment = (std::mem::align_of::<T>() as wgpu::BufferAddress).max(Self::MIN_ALIGNMENT);
        // Pad out the size of the used buffer to a multiple of Self::MIN_ALIGNMENT.
        // This increases our chance of having back to back allocations within a chunk.
        let size = wgpu::util::align_to(
            (std::mem::size_of::<T>() * num_elements) as wgpu::BufferAddress,
            Self::MIN_ALIGNMENT,
        );

        // We need to be super careful with alignment since today wgpu
        // has no guarantees on how pointers to mapped memory are aligned!
        // For all we know, pointers might be 1 aligned, causing even a u32 write to crash the process!
        //
        // For details and (as of writing) ongoing discussion see https://github.com/gfx-rs/wgpu/issues/3508
        //
        // To work around this, we ask for a bigger size, so we can safely pad out
        // if the returned pointer is not correctly aligned.
        // (i.e. we will use _up to_ `required_size` bytes, but at least `size`)]
        let maximum_padding = alignment - 1;
        let max_required_size = size + maximum_padding;

        // Try to find space in any of the active chunks first.
        let mut chunk = if let Some(index) = self
            .active_chunks
            .iter_mut()
            .position(|chunk| chunk.remaining_capacity() >= max_required_size)
        {
            self.active_chunks.swap_remove(index)
        } else {
            self.receive_chunks(); // ensure self.free_chunks is up to date

            // Use a free chunk if possible, fall back to creating a new one if necessary.
            if let Some(index) = self
                .free_chunks
                .iter()
                .position(|chunk| chunk.remaining_capacity() >= max_required_size)
            {
                self.free_chunks.swap_remove(index)
            } else {
                // Allocation might be bigger than a chunk!
                let buffer_size = wgpu::util::align_to(
                    self.chunk_size.max(max_required_size),
                    Self::MIN_ALIGNMENT,
                );
                // Happens relatively rarely, this is a noteworthy event!
                re_log::debug!("Allocating new CpuWriteGpuReadBelt chunk of size {buffer_size}");
                let buffer = buffer_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "CpuWriteGpuReadBelt buffer".into(),
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

        let cpu_buffer_view = chunk.allocate_aligned(num_elements, size, alignment);
        self.active_chunks.push(chunk);
        cpu_buffer_view
    }

    /// Prepare currently mapped buffers for use in a submission.
    ///
    /// This must be called before the command encoder(s) used in [`CpuWriteGpuReadBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed (cannot be used for
    /// further writes) until after [`CpuWriteGpuReadBelt::after_queue_submit`] is called *and* the GPU is done
    /// copying the data from them.
    pub fn before_queue_submit(&mut self) {
        // This would be a great usecase for persistent memory mapping, i.e. mapping without the need to unmap
        // https://github.com/gfx-rs/wgpu/issues/1468
        // However, WebGPU does not support this!

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
    pub fn after_queue_submit(&mut self) {
        self.receive_chunks();

        let sender = &self.sender;
        for chunk in self.closed_chunks.drain(..) {
            let sender = sender.clone();
            chunk
                .buffer
                .clone()
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |_| {
                    let _ = sender.send(chunk);
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
        f.debug_struct("StagingBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("closed_chunks", &self.closed_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .finish_non_exhaustive()
    }
}
