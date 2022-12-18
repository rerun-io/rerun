use std::fmt;
use std::sync::mpsc;

use crate::wgpu_resources::{BufferDesc, GpuBufferHandleStrong, GpuBufferPool};

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBufferHandleStrong,
    /// Size of [`buffer`](#structfield.buffer)
    size: wgpu::BufferAddress,
    /// Begin of unused portion of [`buffer`](#structfield.buffer)
    offset: wgpu::BufferAddress,
}

/// Suballocated staging buffer that can be written to.
pub struct StagingBuffer<'a> {
    buffer_view: wgpu::BufferViewMut<'a>,
}

impl<'a> StagingBuffer<'a> {
    fn copy_to_texture(self, encoder: &mut wgpu::CommandEncoder) {
        //TODO:
    }

    fn copy_to_buffer(self, encoder: &mut wgpu::CommandEncoder) {
        //TODO:
    }
}

/// Efficiently performs many buffer writes by sharing and reusing temporary buffers.
///
/// Internally it uses a ring-buffer of staging buffers that are sub-allocated.
///
/// Based on to [`wgpu::util::StagingBelt`] (https://github.com/gfx-rs/wgpu/blob/a420e453c3d9c93dfb1a8526bf11c000d895c916/wgpu/src/util/belt.rs)
/// Key difference is that it can create buffers without yet knowing the target location.
/// Other difference are alignment guarantees and interop with `re_renderer`'s resource pool system as well as name conventions etc.
pub struct StagingBelt {
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
    sender: mpsc::Sender<Chunk>,
    /// Free chunks are received here to be put on `self.free_chunks`.
    receiver: mpsc::Receiver<Chunk>,
}

impl StagingBelt {
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
    pub fn new(chunk_size: wgpu::BufferAddress) -> Self {
        let (sender, receiver) = mpsc::channel();
        StagingBelt {
            chunk_size,
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Alignment is automatically at least wgpu::MAP_ALIGNMENT
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        resource_pool: &mut GpuBufferPool,
        size: wgpu::BufferAddress,
        alignment: wgpu::BufferAddress,
    ) -> StagingBuffer {
        let mut chunk = if let Some(index) = self
            .active_chunks
            .iter()
            .position(|chunk| chunk.offset + size <= chunk.size)
        {
            self.active_chunks.swap_remove(index)
        } else {
            self.receive_chunks(); // ensure self.free_chunks is up to date

            if let Some(index) = self.free_chunks.iter().position(|chunk| size <= chunk.size) {
                self.free_chunks.swap_remove(index)
            } else {
                let size = self.chunk_size.max(size); // Allocation might be bigger than a chunk
                let buffer = resource_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "StagingBelt buffer".into(),
                        size,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        bypass_reuse_and_map_on_creation: true,
                    },
                );

                Chunk {
                    buffer,
                    size,
                    offset: 0,
                }
            }
        };

        let buffer = resource_pool
            .get_resource(&chunk.buffer)
            .expect("staging buffer missing");

        let old_offset = chunk.offset;
        chunk.offset =
            wgpu::util::align_to(chunk.offset + size, alignment.min(wgpu::MAP_ALIGNMENT));

        // The received chunk is known to be mapped already (either reclaimed or upon creation)
        let buffer_view = buffer.slice(chunk.offset..size).get_mapped_range_mut();

        self.active_chunks.push(chunk);

        StagingBuffer { buffer_view }
    }

    /// Prepare currently mapped buffers for use in a submission.
    ///
    /// This must be called before the command encoder(s) provided to
    /// [`StagingBelt::write_buffer()`] are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed (cannot be used for
    /// further writes) until after [`StagingBelt::recall()`] is called *and* the GPU is done
    /// copying the data from them.
    pub fn finish(&mut self) {
        for chunk in self.active_chunks.drain(..) {
            chunk.buffer.unmap();
            self.closed_chunks.push(chunk);
        }
    }

    /// Recall all of the closed buffers back to be reused.
    ///
    /// This must only be called after the command encoder(s) provided to
    /// [`StagingBelt::write_buffer()`] are submitted. Additional calls are harmless.
    /// Not calling this as soon as possible may result in increased buffer memory usage.
    pub fn recall(&mut self) {
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
            chunk.offset = 0;
            self.free_chunks.push(chunk);
        }
    }
}

impl fmt::Debug for StagingBelt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagingBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("closed_chunks", &self.closed_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .finish_non_exhaustive()
    }
}
