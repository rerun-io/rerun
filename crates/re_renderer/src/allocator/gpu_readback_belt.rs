use std::{num::NonZeroU32, ops::Range, sync::mpsc};

use crate::wgpu_resources::{texture_row_data_info, BufferDesc, GpuBuffer, GpuBufferPool};

pub type GpuReadbackBufferIdentifier = u32;

/// A reserved slice for GPU readback.
///
/// Readback should happen from a buffer/texture with copy-source usage.
/// The `identifier` field is used to identify the buffer upon retrieval of the data in `receive_data`.
pub struct GpuReadbackBuffer {
    chunk_buffer: GpuBuffer,
    byte_offset_in_chunk_buffer: wgpu::BufferAddress,
    size_in_bytes: wgpu::BufferAddress,
    pub identifier: GpuReadbackBufferIdentifier,
}

impl GpuReadbackBuffer {
    /// Populates the buffer with data from a single layer of a 2D texture.
    ///
    /// Implementation note:
    /// Does 2D-only entirely for convenience as it greatly simplifies the input parameters.
    /// Additionally, we assume as tightly as possible packed data as this is by far the most common use.
    pub fn read_texture2d(
        self,
        encoder: &mut wgpu::CommandEncoder,
        source: wgpu::ImageCopyTexture<'_>,
        copy_size_width: u32,
        copy_size_height: u32,
    ) -> GpuReadbackBufferIdentifier {
        let bytes_per_row =
            texture_row_data_info(source.texture.format(), copy_size_width).bytes_per_row_padded;

        // Validate that stay within the slice (wgpu can't fully know our intention here, so we have to check).
        // We go one step further and require the size to be exactly equal - there is no point in reading back more,
        // as this would imply sniffing on unused memory.
        debug_assert_eq!(
            (bytes_per_row * copy_size_height) as u64,
            self.size_in_bytes
        );
        encoder.copy_texture_to_buffer(
            source,
            wgpu::ImageCopyBuffer {
                buffer: &self.chunk_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: self.byte_offset_in_chunk_buffer,
                    bytes_per_row: NonZeroU32::new(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: copy_size_width,
                height: copy_size_height,
                depth_or_array_layers: 1,
            },
        );
        self.identifier
    }

    /// Populates the buffer with data from a buffer.
    pub fn read_buffer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        source: &GpuBuffer,
        source_offset: wgpu::BufferAddress,
    ) -> GpuReadbackBufferIdentifier {
        let copy_size = self.size_in_bytes;

        // Wgpu does validation as well, but in debug mode we want to panic if the buffer doesn't fit.
        debug_assert!(copy_size <= source_offset + source.size(),
            "Source buffer has a size of {}, can't write {copy_size} bytes with an offset of {source_offset}!",
            source.size());

        encoder.copy_buffer_to_buffer(
            source,
            source_offset,
            &self.chunk_buffer,
            self.byte_offset_in_chunk_buffer,
            copy_size,
        );
        self.identifier
    }
}

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBuffer,
    /// All ranges that are currently in use, i.e. there is a GPU write to it scheduled.
    ranges_in_use: Vec<(Range<wgpu::BufferAddress>, GpuReadbackBufferIdentifier)>,
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
        identifier: GpuReadbackBufferIdentifier,
    ) -> GpuReadbackBuffer {
        debug_assert!(size_in_bytes <= self.remaining_capacity());

        let start_offset = self.unused_offset();
        self.ranges_in_use
            .push((start_offset..start_offset + size_in_bytes, identifier));

        GpuReadbackBuffer {
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
pub struct GpuReadbackBelt {
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

    next_identifier: GpuReadbackBufferIdentifier,
}

impl GpuReadbackBelt {
    /// All allocations of this allocator will be aligned to at least this size.
    ///
    /// Buffer mappings however are currently NOT guaranteed to be aligned to this size!
    /// See this issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508).
    const MIN_ALIGNMENT: u64 = wgpu::MAP_ALIGNMENT; //wgpu::COPY_BUFFER_ALIGNMENT.max(wgpu::MAP_ALIGNMENT);

    /// Create a ring buffer for efficient & easy gpu memory readback.
    ///
    /// The `chunk_size` is the unit of internal buffer allocation. Reads will be
    /// sub-allocated within each chunk. Therefore, for optimal use of memory, the
    /// chunk size should be:
    ///
    /// * larger than the largest single [`GpuReadbackBelt::allocate`] operation;
    /// * 1-4 times less than the total amount of data uploaded per submission
    /// * bigger is better, within these bounds.
    ///
    /// TODO(andreas): Adaptive chunk sizes
    /// TODO(andreas): Shrinking after usage spikes?
    pub fn new(chunk_size: wgpu::BufferSize) -> Self {
        let (sender, receiver) = mpsc::channel();
        GpuReadbackBelt {
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
        size_in_bytes: wgpu::BufferAddress,
    ) -> GpuReadbackBuffer {
        crate::profile_function!();

        debug_assert!(size_in_bytes > 0, "Cannot allocate zero-sized buffer");

        let size_in_bytes = wgpu::util::align_to(size_in_bytes, Self::MIN_ALIGNMENT);

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
                // Allocation might be bigger than a chunk!
                let buffer_size = self.chunk_size.max(size_in_bytes);
                // Happens relatively rarely, this is a noteworthy event!
                re_log::debug!("Allocating new GpuReadbackBelt chunk of size {buffer_size}");
                let buffer = buffer_pool.alloc(
                    device,
                    &BufferDesc {
                        label: "GpuReadbackBelt buffer".into(),
                        size: buffer_size,
                        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
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
    /// This should be called before the command encoder(s) used in [`GpuReadbackBuffer`] copy operations are submitted.
    ///
    /// At this point, all the partially used staging buffers are closed until the GPU write operation is done.
    /// After that, the CPU has read them in [`GpuReadbackBelt::receive_data`].
    pub fn after_queue_submit(&mut self) {
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
        mut on_data_received: impl FnMut(&[u8], GpuReadbackBufferIdentifier),
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

impl std::fmt::Debug for GpuReadbackBelt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuReadbackBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .field("next_identifier", &self.next_identifier)
            .finish_non_exhaustive()
    }
}
