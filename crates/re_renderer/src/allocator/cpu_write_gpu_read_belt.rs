use std::{
    ops::DerefMut,
    sync::{mpsc, Arc},
};

use crate::wgpu_resources::{BufferDesc, GpuBufferHandleStrong, GpuBufferPool, PoolError};

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBufferHandleStrong,
    size: usize,
}

/// Chunk that the CPU is writing to currently.
struct ActiveChunk {
    chunk: Chunk,

    /// Write view into the entire buffer.
    ///
    /// UNSAFE: The lifetime is transmuted to be `'static`.
    /// In actuality it is tied to the lifetime of [`buffer`](#structfield.chunk.buffer)!
    write_view: wgpu::BufferViewMut<'static>,

    /// Safety mechanism to track the number of uses of this buffer.
    ///
    /// As long it is bigger 1, we are NOT allowed to deactivate the chunk!
    safety_usage_counter: Arc<()>,

    /// At what offset is [`write_view`](#structfield.write_view) unused.
    unused_offset: usize,
}

impl Chunk {
    #[allow(unsafe_code)]
    fn into_active(self, buffer_pool: &GpuBufferPool) -> ActiveChunk {
        // Get the entire mapped slice up front because mainly because there is no way to know the memory alignment beforehand!
        // After discussing this on the wgpu dev channel, consensus was reached that they should be 4xf32 aligned.
        // TODO: Github issue link.
        //
        // It also safes us a bit of time since we don't have to run through all the checks on every `allocate` call,
        // but this is likely negligible (citation needed).
        //
        // Wgpu isn't fond of that though since it ties the (compile time) lifetime of the returned `wgpu::BufferSlice`
        // to the lifetime of the buffer (after all the mapping should never outlive the buffer!).
        // Therefore, we need to workaround this a bit.
        //
        // Note that in the JavaScript/wasm api of WebGPU this is trivially possible, see
        // https://www.w3.org/TR/webgpu/#dom-gpubuffer-getmappedrange
        // (but it also ends up doing an extra memcpy to that's an unfair comparison)

        let wgpu_buffer = buffer_pool
            .get_resource(&self.buffer)
            .expect("Invalid buffer handle");
        let buffer_slice = wgpu_buffer.slice(..);
        let write_view = buffer_slice.get_mapped_range_mut();

        // SAFETY:
        // Things we need to guarantee manually now:
        // * Buffer needs to stay alive for the time CpuWriteGpuReadBuffer is held.
        //      -> We keep a reference count to `chunk.buffer`
        // * Nobody holds a view into our memory by the time
        //      -> We use `safety_usage_counter` and panic if there is more than one owner when we close the buffer view
        // * Returned ranges into `write_view` NEVER overlap.
        //      -> We track the unused_offset and never return any range ahead of it!
        let write_view = unsafe {
            std::mem::transmute::<wgpu::BufferViewMut<'_>, wgpu::BufferViewMut<'static>>(write_view)
        };

        ActiveChunk {
            chunk: self,
            write_view,
            unused_offset: 0,
            safety_usage_counter: Arc::new(()),
        }
    }
}

// TODO(andreas): Make wgpu::BufferMappedRange Send upstream
#[allow(unsafe_code)]
/// SAFETY:
/// TODO: Link to pr here doing so
unsafe impl std::marker::Send for ActiveChunk {}

/// A suballocated staging buffer that can be written to.
///
/// We do *not* allow reading from this buffer as it is typically write-combined memory.
/// Reading would work, but it can be *very* slow.
pub struct CpuWriteGpuReadBuffer<T: bytemuck::Pod> {
    write_only_memory: &'static mut [T],

    #[allow(dead_code)]
    safety_usage_counter: Arc<()>,

    chunk_buffer: GpuBufferHandleStrong,
    offset_in_chunk_buffer: usize,
}

impl<T> CpuWriteGpuReadBuffer<T>
where
    T: bytemuck::Pod + 'static,
{
    /// Writes several objects to the buffer at a given location.
    /// User is responsible for ensuring the element offset is valid with the element types's alignment requirement.
    /// (panics otherwise)
    ///
    /// We do *not* allow reading from this buffer as it is typically write-combined memory.
    /// Reading would work, but it can be *very* slow.
    #[inline]
    pub fn write(&mut self, elements: impl Iterator<Item = T>, num_elements_offset: usize) {
        for (target, source) in self
            .write_only_memory
            .iter_mut()
            .skip(num_elements_offset)
            .zip(elements)
        {
            *target = source;
        }
    }

    /// Writes a single objects to the buffer at a given location.
    /// User is responsible for ensuring the element offset is valid with the element types's alignment requirement.
    /// (panics otherwise)
    #[inline(always)]
    pub fn write_single(&mut self, element: &T, num_elements_offset: usize) {
        self.write_only_memory[num_elements_offset] = *element;
    }

    // pub fn copy_to_texture(
    //     self,
    //     encoder: &mut wgpu::CommandEncoder,
    //     buffer_pool: &GpuBufferPool,
    //     destination: wgpu::ImageCopyTexture<'_>,
    //     bytes_per_row: Option<NonZeroU32>,
    //     rows_per_image: Option<NonZeroU32>,
    //     copy_size: wgpu::Extent3d,
    // ) -> Result<(), PoolError> {
    //     let buffer = buffer_pool.get_resource(&self.chunk_buffer)?;

    //     encoder.copy_buffer_to_texture(
    //         wgpu::ImageCopyBuffer {
    //             buffer,
    //             layout: wgpu::ImageDataLayout {
    //                 offset: self.offset_in_chunk,
    //                 bytes_per_row,
    //                 rows_per_image,
    //             },
    //         },
    //         destination,
    //         copy_size,
    //     );

    //     Ok(())
    // }

    /// Consume this data view and copy it to another gpu buffer.
    pub fn copy_to_buffer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        buffer_pool: &GpuBufferPool,
        destination: &GpuBufferHandleStrong,
        destination_offset: wgpu::BufferAddress,
    ) -> Result<(), PoolError> {
        encoder.copy_buffer_to_buffer(
            buffer_pool.get_resource(&self.chunk_buffer)?,
            self.offset_in_chunk_buffer as _,
            buffer_pool.get_resource(destination)?,
            destination_offset,
            (self.write_only_memory.len() * std::mem::size_of::<T>()) as u64,
        );
        Ok(())
    }
}

/// Efficiently performs many buffer writes by sharing and reusing temporary buffers.
///
/// Internally it uses a ring-buffer of staging buffers that are sub-allocated.
///
/// Based on to [`wgpu::util::StagingBelt`](https://github.com/gfx-rs/wgpu/blob/a420e453c3d9c93dfb1a8526bf11c000d895c916/wgpu/src/util/belt.rs)
/// However, there are some important differences:
/// * can create buffers without yet knowing the target copy location
/// * lifetime of returned buffers is independent of the [`StagingWriteBelt`] (allows working with several in parallel!)
/// * use of `re_renderer`'s resource pool
/// * handles alignment
pub struct CpuWriteGpuReadBelt {
    /// Minimum size for new buffers.
    chunk_size: usize,

    /// Chunks which are CPU write at the moment.
    active_chunks: Vec<ActiveChunk>,

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
    /// Behind a mutex, so that our StagingBelt becomes Sync.
    /// Note that we shouldn't use SyncSender since this can block the Sender if a buffer is full,
    /// which means that in a single threaded situation (Web!) we might deadlock.
    sender: mpsc::Sender<Chunk>,

    /// Free chunks are received here to be put on `self.free_chunks`.
    receiver: mpsc::Receiver<Chunk>,
}

impl CpuWriteGpuReadBelt {
    /// Create a new staging belt.
    ///
    /// The `chunk_size` is the unit of internal buffer allocation; writes will be
    /// sub-allocated within each chunk. Therefore, for optimal use of memory, the
    /// chunk size should be:
    ///
    /// * larger than the largest single [`StagingBelt::write_buffer()`] operation;
    /// * 1-4 times less than the total amount of data uploaded per submission
    ///   (per [`StagingBelt::finish()`]); and
    /// * bigger is better, within these bounds.
    ///
    /// TODO(andreas): Adaptive chunk sizes
    /// TODO(andreas): Shrinking after usage spikes?
    pub fn new(chunk_size: usize) -> Self {
        let (sender, receiver) = mpsc::channel();
        CpuWriteGpuReadBelt {
            chunk_size,
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Allocates a cpu writable buffer for `num_elements` instances of T.
    ///
    /// Handles alignment requirements automatically which allows faster copy operations.
    #[allow(unsafe_code)]
    pub fn allocate<T: bytemuck::Pod + 'static>(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &mut GpuBufferPool,
        num_elements: usize,
    ) -> CpuWriteGpuReadBuffer<T> {
        let alignment = std::mem::align_of::<T>().min(wgpu::MAP_ALIGNMENT as usize);
        let size = std::mem::size_of::<T>() * num_elements;

        // We need to be super careful with alignment since wgpu
        // has no guarantees on how pointers to mapped memory are aligned!

        // We explicitly use `deref_mut` on write_view everywhere, since wgpu warns if we accidentally use `deref`.

        // Try to find space in any of the active chunks first.
        let mut active_chunk = if let Some(index) =
            self.active_chunks.iter_mut().position(|active_chunk| {
                size + (active_chunk.write_view.deref_mut().as_ptr() as usize
                    + active_chunk.unused_offset)
                    % alignment
                    <= active_chunk.chunk.size - active_chunk.unused_offset
            }) {
            self.active_chunks.swap_remove(index)
        } else {
            self.receive_chunks(); // ensure self.free_chunks is up to date

            // We don't know yet how aligned the mapped pointer is. So we need to ask for more memory!
            let required_size = size + alignment - 1;

            // Use a free chunk if possible, fall back to creating a new one if necessary.
            if let Some(index) = self
                .free_chunks
                .iter()
                .position(|chunk| chunk.size >= required_size)
            {
                self.free_chunks.swap_remove(index).into_active(buffer_pool)
            } else {
                // Allocation might be bigger than a chunk.
                let size = self.chunk_size.max(required_size);

                let buffer = buffer_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "CpuWriteGpuReadBelt buffer".into(),
                        size: size as _,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: true,
                    },
                );

                Chunk { buffer, size }.into_active(buffer_pool)
            }
        };

        let alignment_padding = (active_chunk.write_view.deref_mut().as_ptr() as usize
            + active_chunk.unused_offset)
            % alignment;
        let start_offset = active_chunk.unused_offset + alignment_padding;
        let end_offset = start_offset + size;

        let write_only_memory = bytemuck::cast_slice_mut(
            &mut active_chunk.write_view.deref_mut()[start_offset..end_offset],
        );

        // SAFETY:
        // The slice we take out of the write view has a lifetime dependency on the write_view.
        // This means it is self-referential. We handle this lifetime at runtime instead by
        //
        // See also `ActiveChunk::into_active`.
        let write_only_memory =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(write_only_memory) };
        let cpu_buffer_view = CpuWriteGpuReadBuffer {
            write_only_memory,
            safety_usage_counter: active_chunk.safety_usage_counter.clone(),
            chunk_buffer: active_chunk.chunk.buffer.clone(),
            offset_in_chunk_buffer: start_offset,
        };

        self.active_chunks.push(active_chunk);

        cpu_buffer_view
    }

    /// Prepare currently mapped buffers for use in a submission.
    ///
    /// This must be called before the command encoder(s) used in [`StagingBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed (cannot be used for
    /// further writes) until after [`StagingBelt::recall()`] is called *and* the GPU is done
    /// copying the data from them.
    pub fn before_queue_submit(&mut self, buffer_pool: &GpuBufferPool) {
        // This would be a great usecase for persistent memory mapping, i.e. mapping without the need to unmap
        // https://github.com/gfx-rs/wgpu/issues/1468
        // However, WebGPU does not support this!

        for active_chunk in self.active_chunks.drain(..) {
            assert!(Arc::strong_count(&active_chunk.safety_usage_counter) == 1,
                "Chunk still in use. All instances of `CpuWriteGpuReadBelt` need to be freed before submitting.");

            // Ensure write view is dropped before we unmap!
            drop(active_chunk.write_view);

            buffer_pool
                .get_resource(&active_chunk.chunk.buffer)
                .expect("invalid buffer handle")
                .unmap();
            self.closed_chunks.push(active_chunk.chunk);
        }
    }

    /// Recall all of the closed buffers back to be reused.
    ///
    /// This must only be called after the command encoder(s) used in [`StagingBuffer`]
    /// copy operations are submitted. Additional calls are harmless.
    /// Not calling this as soon as possible may result in increased buffer memory usage.
    pub fn after_queue_submit(&mut self, buffer_pool: &GpuBufferPool) {
        self.receive_chunks();

        let sender = &self.sender;
        for chunk in self.closed_chunks.drain(..) {
            let sender = sender.clone();
            buffer_pool
                .get_resource(&chunk.buffer)
                .expect("invalid buffer handle")
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |_| {
                    let _ = sender.send(chunk);
                });
        }
    }

    /// Move all chunks that the GPU is done with (and are now mapped again)
    /// from `self.receiver` to `self.free_chunks`.
    fn receive_chunks(&mut self) {
        while let Ok(chunk) = self.receiver.try_recv() {
            self.free_chunks.push(chunk);
        }
    }
}

// impl fmt::Debug for StagingBelt {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_struct("StagingBelt")
//             .field("chunk_size", &self.chunk_size)
//             .field("active_chunks", &self.active_chunks.len())
//             .field("closed_chunks", &self.closed_chunks.len())
//             .field("free_chunks", &self.free_chunks.len())
//             .finish_non_exhaustive()
//     }
// }
