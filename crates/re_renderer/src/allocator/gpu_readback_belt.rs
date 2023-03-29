use std::{num::NonZeroU32, ops::Range, sync::mpsc};

use crate::wgpu_resources::{BufferDesc, GpuBuffer, GpuBufferPool, TextureRowDataInfo};

pub type GpuReadbackIdentifier = u64;
pub type GpuReadbackUserData = Box<dyn std::any::Any + 'static + Send>;

struct PendingReadbackRange {
    identifier: GpuReadbackIdentifier,
    buffer_range: Range<wgpu::BufferAddress>,
    user_data: GpuReadbackUserData,
}

/// A reserved slice for GPU readback.
///
/// Readback should happen from a buffer/texture with copy-source usage.
/// The `identifier` field is used to identify the buffer upon retrieval of the data in `receive_data`.
pub struct GpuReadbackBuffer {
    chunk_buffer: GpuBuffer,
    range_in_chunk: Range<wgpu::BufferAddress>,
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
        copy_extents: glam::UVec2,
    ) {
        self.read_multiple_texture2d(encoder, &[(source, copy_extents)])
    }

    /// Reads multiple textures into the same buffer.
    ///
    /// This is primarily useful if you need to make sure that data from all textures is available at the same time.
    ///
    /// Special care has to be taken to ensure that the buffer is large enough.
    ///
    /// ATTENTION: Keep in mind that internal offsets need to be a multiple of the texture block size!
    /// While readback buffer starts are guaranteed to be aligned correctly, there might need to be extra padding needed between texture copies.
    /// This method will add the required padding between the texture copies if necessary.
    /// Panics if the buffer is too small.
    pub fn read_multiple_texture2d(
        mut self,
        encoder: &mut wgpu::CommandEncoder,
        sources_and_extents: &[(wgpu::ImageCopyTexture<'_>, glam::UVec2)],
    ) {
        for (source, copy_extents) in sources_and_extents {
            let start_offset = wgpu::util::align_to(
                self.range_in_chunk.start,
                source.texture.format().describe().block_size as u64,
            );

            let bytes_per_row = TextureRowDataInfo::new(source.texture.format(), copy_extents.x)
                .bytes_per_row_padded;
            let num_bytes = bytes_per_row * copy_extents.y;

            // Validate that stay within the slice (wgpu can't fully know our intention here, so we have to check).
            debug_assert!(
                (num_bytes as u64) <= self.range_in_chunk.end - start_offset,
                "Texture data is too large to fit into the readback buffer!"
            );

            encoder.copy_texture_to_buffer(
                source.clone(),
                wgpu::ImageCopyBuffer {
                    buffer: &self.chunk_buffer,
                    layout: wgpu::ImageDataLayout {
                        offset: start_offset,
                        bytes_per_row: NonZeroU32::new(bytes_per_row),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: copy_extents.x,
                    height: copy_extents.y,
                    depth_or_array_layers: 1,
                },
            );

            self.range_in_chunk = start_offset..self.range_in_chunk.end;
        }
    }

    /// Populates the buffer with data from a buffer.
    ///
    /// Panics if the readback buffer is too small to fit the data.
    pub fn read_buffer(
        self,
        encoder: &mut wgpu::CommandEncoder,
        source: &GpuBuffer,
        source_offset: wgpu::BufferAddress,
    ) {
        let copy_size = self.range_in_chunk.end - self.range_in_chunk.start;

        // Wgpu does validation as well, but in debug mode we want to panic if the buffer doesn't fit.
        debug_assert!(copy_size <= source_offset + source.size(),
            "Source buffer has a size of {}, can't write {copy_size} bytes with an offset of {source_offset}!",
            source.size());

        encoder.copy_buffer_to_buffer(
            source,
            source_offset,
            &self.chunk_buffer,
            self.range_in_chunk.start,
            copy_size,
        );
    }
}

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBuffer,

    /// Offset from which on the buffer is unused.
    unused_offset: wgpu::BufferAddress,

    /// All ranges that are currently in use, i.e. there is a GPU write to it scheduled.
    ranges_in_use: Vec<PendingReadbackRange>,
}

impl Chunk {
    fn remaining_capacity(&self) -> wgpu::BufferAddress {
        self.buffer.size() - self.unused_offset
    }

    /// Caller needs to make sure that there is enough space.
    fn allocate(
        &mut self,
        size_in_bytes: wgpu::BufferAddress,
        identifier: GpuReadbackIdentifier,
        user_data: GpuReadbackUserData,
    ) -> GpuReadbackBuffer {
        debug_assert!(size_in_bytes <= self.remaining_capacity());

        let buffer_range = self.unused_offset..self.unused_offset + size_in_bytes;

        self.ranges_in_use.push(PendingReadbackRange {
            identifier,
            buffer_range: buffer_range.clone(),
            user_data,
        });

        let buffer = GpuReadbackBuffer {
            chunk_buffer: self.buffer.clone(),
            range_in_chunk: buffer_range,
        };

        self.unused_offset += size_in_bytes;

        buffer
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

    /// Chunks that have been unmapped and are ready for writing by the GPU.
    free_chunks: Vec<Chunk>,

    /// Chunks that are currently mapped and ready for reading by the CPU.
    pending_chunks: Vec<Chunk>,

    /// When a chunk mapping is successful, it is moved to this sender to be read by the CPU.
    sender: mpsc::Sender<Chunk>,

    /// Chunks are received here are ready to be read by the CPU.
    receiver: mpsc::Receiver<Chunk>,
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
            pending_chunks: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Allocates a Gpu writable buffer & cpu readable buffer with a given size.
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &GpuBufferPool,
        size_in_bytes: wgpu::BufferAddress,
        identifier: GpuReadbackIdentifier,
        user_data: GpuReadbackUserData,
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
                        label: "GpuReadbackBelt chunk buffer".into(),
                        size: buffer_size,
                        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                );

                Chunk {
                    buffer,
                    unused_offset: 0,
                    ranges_in_use: Vec::new(),
                }
            }
        };

        let buffer_slice = chunk.allocate(size_in_bytes, identifier, user_data);
        self.active_chunks.push(chunk);

        buffer_slice
    }

    /// Prepare used buffers for CPU read.
    ///
    /// This should be called after the command encoder(s) used in [`GpuReadbackBuffer`] copy operations are submitted.
    pub fn after_queue_submit(&mut self) {
        crate::profile_function!();
        for chunk in self.active_chunks.drain(..) {
            let sender = self.sender.clone();
            chunk.buffer.clone().slice(..chunk.unused_offset).map_async(
                wgpu::MapMode::Read,
                move |result| {
                    if result.is_err() {
                        // This should never happen. Drop the chunk and report.
                        re_log::error_once!("Failed to map staging buffer for reading");
                    } else {
                        let _ = sender.send(chunk);
                    }
                },
            );
        }
    }

    /// Try to receive a pending data readback with the given identifier.
    ///
    /// If several pieces of data have the same identifier, only the callback is invoked only on the oldest received.
    /// (which is typically the oldest scheduled as well, but there is not strict guarantee for this)
    ///
    /// ATTENTION: Do NOT assume any alignment on the slice passed to `on_data_received`.
    /// See this issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508).
    pub fn readback_data<UserDataType: 'static>(
        &mut self,
        identifier: GpuReadbackIdentifier,
        callback: impl FnOnce(&[u8], Box<UserDataType>),
    ) {
        crate::profile_function!();

        // Check if any new chunks are ready to be read.
        while let Ok(chunk) = self.receiver.try_recv() {
            self.pending_chunks.push(chunk);
        }

        // Search for the user data in the readback chunks.
        // A linear search is suited best since we expect the number both the number of pending chunks
        // (typically just one or two!)
        // as well as the number of readbacks per chunk to be small!
        // Also note that identifiers may not be unique!
        for (chunk_index, chunk) in self.pending_chunks.iter_mut().enumerate() {
            for (range_index, range) in chunk.ranges_in_use.iter().enumerate() {
                if range.identifier != identifier || !range.user_data.is::<UserDataType>() {
                    continue;
                }

                {
                    let range = chunk.ranges_in_use.swap_remove(range_index);
                    let slice = chunk.buffer.slice(range.buffer_range.clone());
                    let data = slice.get_mapped_range();
                    callback(&data, range.user_data.downcast::<UserDataType>().unwrap());
                }

                // If this was the last range from this chunk, the chunk is ready for re-use!
                if chunk.ranges_in_use.is_empty() {
                    let chunk = self.pending_chunks.swap_remove(chunk_index);
                    chunk.buffer.unmap();
                    self.free_chunks.push(chunk);
                }
                return;
            }
        }
    }

    // TODO: Have a GC mechanism that deals with chunks that have lingering data.
}

impl std::fmt::Debug for GpuReadbackBelt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuReadbackBelt")
            .field("chunk_size", &self.chunk_size)
            .field("active_chunks", &self.active_chunks.len())
            .field("free_chunks", &self.free_chunks.len())
            .finish_non_exhaustive()
    }
}
