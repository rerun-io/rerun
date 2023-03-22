use std::{num::NonZeroU32, ops::Range, sync::mpsc};

use crate::wgpu_resources::{BufferDesc, GpuBuffer, GpuBufferPool};

pub type GpuWriteCpuReadBufferIdentifier = u32;

/// TODO: Docstring
pub struct GpuWriteCpuReadBuffer {
    chunk_buffer: GpuBuffer,
    byte_offset_in_chunk_buffer: wgpu::BufferAddress,
    size_in_bytes: wgpu::BufferAddress,
    identifier: GpuWriteCpuReadBufferIdentifier,
}

impl GpuWriteCpuReadBuffer {
    /// Populates the buffer with data from a texture.
    pub fn read_texture(
        self,
        encoder: &mut wgpu::CommandEncoder,
        source: wgpu::ImageCopyTexture<'_>,
        bytes_per_row: Option<NonZeroU32>,
        rows_per_image: Option<NonZeroU32>,
        copy_size: wgpu::Extent3d,
    ) -> GpuWriteCpuReadBufferIdentifier {
        // TODO: validate that stay within the slice.
        encoder.copy_texture_to_buffer(
            source,
            wgpu::ImageCopyBuffer {
                buffer: &self.chunk_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: self.byte_offset_in_chunk_buffer,
                    bytes_per_row,
                    rows_per_image,
                },
            },
            copy_size,
        );
        self.identifier
    }

    /// Populates the buffer with data from a buffer.
    pub fn read_buffer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        source: &GpuBuffer,
        source_offset: wgpu::BufferAddress,
    ) -> GpuWriteCpuReadBufferIdentifier {
        encoder.copy_buffer_to_buffer(
            source,
            source_offset,
            &self.chunk_buffer,
            self.byte_offset_in_chunk_buffer,
            self.size_in_bytes,
        );
        self.identifier
    }
}

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBuffer,
    /// All ranges that are currently in use, i.e. there is a GPU write to it scheduled.
    ranges_in_use: Vec<(Range<wgpu::BufferAddress>, GpuWriteCpuReadBufferIdentifier)>,
}

impl Chunk {
    fn unused_offset(&self) -> wgpu::BufferAddress {
        self.ranges_in_use.last().map_or(0, |(range, _)| range.end)
    }

    fn remaining_capacity(&self) -> wgpu::BufferAddress {
        self.buffer.size() - self.unused_offset()
    }

    /// Caller needs to make sure that there is enough space.
    fn allocate(
        &mut self,
        size_in_bytes: wgpu::BufferAddress,
        identifier: GpuWriteCpuReadBufferIdentifier,
    ) -> GpuWriteCpuReadBuffer {
        debug_assert!(size_in_bytes <= self.remaining_capacity());

        let start_offset = self.unused_offset();
        self.ranges_in_use
            .push((start_offset..start_offset + size_in_bytes, identifier));

        GpuWriteCpuReadBuffer {
            chunk_buffer: self.buffer.clone(),
            byte_offset_in_chunk_buffer: start_offset,
            size_in_bytes,
            identifier,
        }
    }
}

/// Efficiently performs many buffer reads by sharing and reusing temporary buffers.
///
/// Internally it uses a ring-buffer of staging buffers that are sub-allocated.
pub struct GpuWriteCpuReadBelt {
    /// Minimum size for new buffers.
    chunk_size: u64,

    /// Chunks which the GPU writes are scheduled, but we haven't mapped them yet.
    active_chunks: Vec<Chunk>,

    /// Chunks that have been mapped by the CPU and are
    free_chunks: Vec<Chunk>,

    /// When a chunk mapping is successful, it is moved to this sender to be read by the CPU.
    sender: mpsc::Sender<Chunk>,

    /// Chunks are received here are ready to be read by the CPU.
    receiver: mpsc::Receiver<Chunk>,

    next_identifier: GpuWriteCpuReadBufferIdentifier,
}

impl GpuWriteCpuReadBelt {
    /// All allocations of this allocator will be aligned to at least this size.
    ///
    /// Buffer mappings however are currently NOT guaranteed to be aligned to this size!
    /// See this issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508).
    const MIN_ALIGNMENT: u64 = wgpu::MAP_ALIGNMENT; //wgpu::COPY_BUFFER_ALIGNMENT.max(wgpu::MAP_ALIGNMENT);

    /// Create a gpu-write & cpu-read staging belt.
    ///
    /// The `chunk_size` is the unit of internal buffer allocation; writes will be
    /// sub-allocated within each chunk. Therefore, for optimal use of memory, the
    /// chunk size should be:
    ///
    /// * larger than the largest single [`GpuWriteCpuReadBelt::allocate`] operation;
    /// * 1-4 times less than the total amount of data uploaded per submission
    ///   (per [`GpuWriteCpuReadBelt::before_queue_submit()`]); and
    /// * bigger is better, within these bounds.
    ///
    /// TODO(andreas): Adaptive chunk sizes
    /// TODO(andreas): Shrinking after usage spikes?
    pub fn new(chunk_size: wgpu::BufferSize) -> Self {
        let (sender, receiver) = mpsc::channel();
        GpuWriteCpuReadBelt {
            chunk_size: wgpu::util::align_to(chunk_size.get(), Self::MIN_ALIGNMENT),
            active_chunks: Vec::new(),
            free_chunks: Vec::new(),
            sender,
            receiver,
            next_identifier: 0,
        }
    }

    /// Allocates a Gpu writable buffer & cpu readable buffer with a given size.
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &GpuBufferPool,
        size_in_bytes: wgpu::BufferSize,
    ) -> GpuWriteCpuReadBuffer {
        crate::profile_function!();

        let size_in_bytes = wgpu::util::align_to(size_in_bytes.get(), Self::MIN_ALIGNMENT);

        // Try to find space in any of the active chunks first.
        let mut chunk = if let Some(index) = self
            .active_chunks
            .iter_mut()
            .position(|chunk| chunk.remaining_capacity() >= size_in_bytes)
        {
            self.active_chunks.swap_remove(index)
        } else {
            // Use a free chunk if possible, fall back to creating a new one if necessary.
            if let Some(index) = self
                .free_chunks
                .iter()
                .position(|chunk| chunk.remaining_capacity() >= size_in_bytes)
            {
                self.free_chunks.swap_remove(index)
            } else {
                // Happens relatively rarely, this is a noteworthy event!
                re_log::debug!("Allocating new GpuWriteCpuReadBelt chunk of size {size_in_bytes}");
                let buffer = buffer_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "GpuWriteCpuReadBelt buffer".into(),
                        size: size_in_bytes,
                        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: true,
                    },
                );

                Chunk {
                    buffer,
                    ranges_in_use: Vec::new(),
                }
            }
        };

        let buffer_slice = chunk.allocate(size_in_bytes, self.next_identifier);
        self.active_chunks.push(chunk);

        self.next_identifier += 1;

        buffer_slice
    }

    /// Prepare used buffers for CPU read.
    ///
    /// This should be called before the command encoder(s) used in [`GpuWriteCpuReadBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed until the GPU write operation is done and the CPU has read them.
    pub fn before_queue_submit(&mut self) {
        crate::profile_function!();
        for chunk in self.active_chunks.drain(..) {
            let sender = self.sender.clone();
            chunk
                .buffer
                .clone()
                .slice(..chunk.unused_offset())
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_err() {
                        // This should never happen. Drop the chunk and report.
                        re_log::error_once!("Failed to map staging buffer for reading");
                    } else {
                        let _ = sender.send(chunk);
                    }
                });
        }
    }

    /// Receive all buffers that have been written by the GPU and are ready to be read by the CPU.
    ///
    /// ATTENTION: Do NOT assume any alignment on the slice passed to `on_data_received`.
    /// See this issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508).
    ///
    /// This should be called every frame to ensure that we're not accumulating too many buffers.
    /// After this call, internal chunks can be re-used.
    pub fn receive_data(
        &mut self,
        on_data_received: impl Fn(&[u8], GpuWriteCpuReadBufferIdentifier),
    ) {
        crate::profile_function!();

        while let Ok(mut chunk) = self.receiver.try_recv() {
            {
                let buffer_view = chunk
                    .buffer
                    .slice(..chunk.unused_offset())
                    .get_mapped_range();

                for (range, identifier) in chunk.ranges_in_use.drain(..) {
                    on_data_received(
                        &buffer_view[range.start as usize..range.end as usize],
                        identifier,
                    );
                }
            }

            // Ready for re-use!
            chunk.buffer.unmap();
            self.free_chunks.push(chunk);
        }
    }
}

impl std::fmt::Debug for GpuWriteCpuReadBelt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuWriteCpuReadBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .field("next_identifier", &self.next_identifier)
            .finish_non_exhaustive()
    }
}
