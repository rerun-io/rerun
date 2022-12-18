use std::{num::NonZeroU32, sync::mpsc};

use crate::wgpu_resources::{GpuBufferPool, StagingWriteBuffer};

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: StagingWriteBuffer,

    /// Begin of unused portion of [`buffer`](#structfield.buffer)
    unused_offset: wgpu::BufferAddress,
}

/// Suballocated staging buffer that can be written to.
///
/// We do *not* allow reading from this buffer as it is typically write-combined memory.
/// Reading would work, but it can be *insanely* slow.
pub struct StagingWriteBeltBuffer {
    /// Chunk this buffer originated from, need to keep it around so buffer_view is not invalidated.
    chunk_buffer: StagingWriteBuffer,

    write_view: wgpu::BufferViewMut<'static>,

    offset_in_chunk: wgpu::BufferAddress,
}

impl StagingWriteBeltBuffer {
    /// Writes bytes to the buffer at a given location.
    ///
    /// We do *not* allow reading from this buffer as it is typically write-combined memory.
    /// Reading would work, but it can be *insanely* slow.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8], offset: usize) {
        self.write_view[offset..(offset + bytes.len())].clone_from_slice(bytes);
    }

    /// Writes several objects to the buffer at a given location.
    /// User is responsible for ensuring the element offset is valid with the element types's alignment requirement.
    /// (panics otherwise)
    ///
    /// We do *not* allow reading from this buffer as it is typically write-combined memory.
    /// Reading would work, but it can be *insanely* slow.
    #[inline]
    pub fn write<T: bytemuck::Pod>(&mut self, elements: &[T], offset_in_element_sizes: usize) {
        bytemuck::cast_slice_mut(&mut self.write_view)
            [offset_in_element_sizes..(offset_in_element_sizes + elements.len())]
            .clone_from_slice(elements);
    }

    /// Writes a single objects to the buffer at a given location.
    /// User is responsible for ensuring the element offset is valid with the element types's alignment requirement.
    /// (panics otherwise)
    #[inline]
    pub fn write_single<T: bytemuck::Pod>(&mut self, element: &T, offset_in_element_sizes: usize) {
        bytemuck::cast_slice_mut(&mut self.write_view)[offset_in_element_sizes] = *element;
    }

    /// Sets all bytes in the buffer to a given value
    pub fn memset(&mut self, value: u8) {
        self.write_view.fill(value);
    }

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
                    offset: self.offset_in_chunk,
                    bytes_per_row,
                    rows_per_image,
                },
            },
            destination,
            copy_size,
        );
    }

    // TODO(andreas):
    // fn copy_to_buffer(self, encoder: &mut wgpu::CommandEncoder) {
    // }
}

/// Efficiently performs many buffer writes by sharing and reusing temporary buffers.
///
/// Internally it uses a ring-buffer of staging buffers that are sub-allocated.
///
/// Based on to [`wgpu::util::StagingBelt`](https://github.com/gfx-rs/wgpu/blob/a420e453c3d9c93dfb1a8526bf11c000d895c916/wgpu/src/util/belt.rs)
/// However, there are some important differences:
/// * can create buffers without yet knowing the target copy location
/// * lifetime of returned buffers is independent of the StagingBelt (allows working with several in parallel!)
/// * use of re_renderer's resource pool
pub struct StagingWriteBelt {
    chunk_size: wgpu::BufferAddress,
    /// Chunks into which we are accumulating data to be transferred.
    active_chunks: Vec<Chunk>,
    /// Chunks that have scheduled transfers already; they are unmapped and some
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

impl StagingWriteBelt {
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
    pub fn new(chunk_size: wgpu::BufferAddress) -> Self {
        let (sender, receiver) = mpsc::channel();
        StagingWriteBelt {
            chunk_size,
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Alignment is automatically at least [`wgpu::MAP_ALIGNMENT`]
    #[allow(unsafe_code)]
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &mut GpuBufferPool,
        size: wgpu::BufferAddress,
        alignment: wgpu::BufferAddress,
    ) -> StagingWriteBeltBuffer {
        let mut chunk = if let Some(index) = self
            .active_chunks
            .iter()
            .position(|chunk| chunk.unused_offset + size <= chunk.buffer.size())
        {
            self.active_chunks.swap_remove(index)
        } else {
            self.receive_chunks(); // ensure self.free_chunks is up to date

            if let Some(index) = self
                .free_chunks
                .iter()
                .position(|chunk| size <= chunk.buffer.size())
            {
                self.free_chunks.swap_remove(index)
            } else {
                let size = self.chunk_size.max(size); // Allocation might be bigger than a chunk
                let buffer = buffer_pool.alloc_staging_write_buffer(
                    device,
                    "StagingBelt buffer".into(),
                    size,
                );

                Chunk {
                    buffer,
                    unused_offset: 0,
                }
            }
        };

        let start_offset = wgpu::util::align_to(
            chunk.unused_offset + size,
            alignment.min(wgpu::MAP_ALIGNMENT),
        );
        let end_offset = start_offset + size;
        chunk.unused_offset = end_offset;

        self.active_chunks.push(chunk);
        let chunk = self.active_chunks.last_mut().unwrap();

        let buffer_ptr = &mut chunk.buffer as *mut StagingWriteBuffer;

        // The received chunk is known to be mapped already (either reclaimed or upon creation)
        // Note that get_mapped_range_mut will internally check if we give out overlapping ranges.
        // TODO: explain why this is ok
        let static_buffer = Box::leak(unsafe { Box::from_raw(buffer_ptr) });

        let write_view = static_buffer
            .slice(start_offset..end_offset)
            .get_mapped_range_mut();

        let chunk_buffer = chunk.buffer.clone();
        StagingWriteBeltBuffer {
            chunk_buffer,
            offset_in_chunk: start_offset,
            write_view,
        }
    }

    /// Prepare currently mapped buffers for use in a submission.
    ///
    /// This must be called before the command encoder(s) used in [`StagingBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed (cannot be used for
    /// further writes) until after [`StagingBelt::recall()`] is called *and* the GPU is done
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
    /// This must only be called after the command encoder(s) used in [`StagingBuffer`]
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
