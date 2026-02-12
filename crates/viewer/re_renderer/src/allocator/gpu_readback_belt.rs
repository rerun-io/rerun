use std::ops::Range;
use std::sync::mpsc;

use re_log::debug_assert;

use crate::texture_info::Texture2DBufferInfo;
use crate::wgpu_resources::{BufferDesc, GpuBuffer, GpuBufferPool};

/// Identifier used to identify a buffer upon retrieval of the data.
///
/// Does not need to be unique!
pub type GpuReadbackIdentifier = u64;

/// Type used for storing user data on the gpu readback belt.
pub type GpuReadbackUserDataStorage = Box<dyn std::any::Any + 'static + Send>;

struct PendingReadbackRange {
    identifier: GpuReadbackIdentifier,
    buffer_range: Range<wgpu::BufferAddress>,
    user_data: GpuReadbackUserDataStorage,

    /// The frame index when the readback was scheduled.
    scheduled_frame_index: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum GpuReadbackError {
    #[error("Texture format {0:?} is not supported for readback.")]
    UnsupportedTextureFormatForReadback(wgpu::TextureFormat),

    #[error("Texture or buffer does not have the required copy-source usage flag.")]
    MissingSrcCopyUsage,
}

/// A reserved slice for GPU readback.
///
/// Readback needs to happen from a buffer/texture with copy-source usage,
/// as we need to copy the data from the GPU to this CPU accessible buffer.
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
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        source: wgpu::TexelCopyTextureInfo<'_>,
        copy_extents: wgpu::Extent3d,
    ) -> Result<(), GpuReadbackError> {
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
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        sources_and_extents: &[(wgpu::TexelCopyTextureInfo<'_>, wgpu::Extent3d)],
    ) -> Result<(), GpuReadbackError> {
        for (source, copy_extents) in sources_and_extents {
            let src_texture = source.texture;
            if !src_texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
                return Err(GpuReadbackError::MissingSrcCopyUsage);
            }

            let start_offset = wgpu::util::align_to(
                self.range_in_chunk.start,
                src_texture
                    .format()
                    .block_copy_size(Some(source.aspect))
                    .ok_or_else(|| {
                        GpuReadbackError::UnsupportedTextureFormatForReadback(
                            source.texture.format(),
                        )
                    })? as u64,
            );

            let buffer_info = Texture2DBufferInfo::new(src_texture.format(), *copy_extents);

            // Validate that stay within the slice (wgpu can't fully know our intention here, so we have to check).
            debug_assert!(
                buffer_info.buffer_size_padded <= self.range_in_chunk.end - start_offset,
                "Texture data is too large to fit into the readback buffer!"
            );

            encoder.copy_texture_to_buffer(
                *source,
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.chunk_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: start_offset,
                        bytes_per_row: Some(buffer_info.bytes_per_row_padded),
                        rows_per_image: None,
                    },
                },
                *copy_extents,
            );

            self.range_in_chunk =
                (start_offset + buffer_info.buffer_size_padded)..self.range_in_chunk.end;
        }
        Ok(())
    }

    // TODO(andreas): Unused & untested so far!
    //
    // Populates the buffer with data from a buffer.
    //
    // Panics if the readback buffer is too small to fit the data.
    // pub fn read_buffer(
    //     self,
    //     encoder: &mut wgpu::CommandEncoder,
    //     source: &GpuBuffer,
    //     source_offset: wgpu::BufferAddress,
    // ) {
    //     let copy_size = self.range_in_chunk.end - self.range_in_chunk.start;

    //     // Wgpu does validation as well, but in debug mode we want to panic if the buffer doesn't fit.
    //     debug_assert!(copy_size <= source_offset + source.size(),
    //         "Source buffer has a size of {}, can't write {copy_size} bytes with an offset of {source_offset}!",
    //         source.size());

    //     encoder.copy_buffer_to_buffer(
    //         source,
    //         source_offset,
    //         &self.chunk_buffer,
    //         self.range_in_chunk.start,
    //         copy_size,
    //     );
    // }
}

/// Internal chunk of the staging belt.
struct Chunk {
    buffer: GpuBuffer,

    /// Offset from which on the buffer is unused.
    unused_offset: wgpu::BufferAddress,

    /// All ranges that are currently in use, i.e. there is a GPU write to it scheduled.
    ranges_in_use: Vec<PendingReadbackRange>,

    /// Last frame this chunk was received, i.e. the last time a `map_async` action operation finished with it.
    last_received_frame_index: u64,
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
        user_data: GpuReadbackUserDataStorage,
        scheduled_frame_index: u64,
    ) -> GpuReadbackBuffer {
        debug_assert!(size_in_bytes <= self.remaining_capacity());

        let buffer_range = self.unused_offset..self.unused_offset + size_in_bytes;

        self.ranges_in_use.push(PendingReadbackRange {
            identifier,
            buffer_range: buffer_range.clone(),
            user_data,
            scheduled_frame_index,
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

    /// Chunks for which the GPU writes are scheduled, but we haven't mapped them yet.
    active_chunks: Vec<Chunk>,

    /// Chunks that have been unmapped and are ready for writing by the GPU.
    free_chunks: Vec<Chunk>,

    /// Chunks that are currently mapped and ready for reading by the CPU.
    received_chunks: Vec<Chunk>,

    /// When a chunk mapping is successful, it is moved to this sender to be read by the CPU.
    sender: mpsc::Sender<Chunk>,

    /// Chunks are received here are ready to be read by the CPU.
    receiver: mpsc::Receiver<Chunk>,

    /// Current frame index, used for keeping track of how old chunks are.
    frame_index: u64,
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
    /// TODO(andreas): Shrinking after usage spikes (e.g. screenshots of different sizes!)
    pub fn new(chunk_size: wgpu::BufferSize) -> Self {
        // we must use an unbounded channel to avoid blocking on web
        #[expect(clippy::disallowed_methods)]
        let (sender, receiver) = mpsc::channel();
        Self {
            chunk_size: wgpu::util::align_to(chunk_size.get(), Self::MIN_ALIGNMENT),
            active_chunks: Vec::new(),
            free_chunks: Vec::new(),
            received_chunks: Vec::new(),
            sender,
            receiver,
            frame_index: 0,
        }
    }

    /// Allocates a Gpu writable buffer & cpu readable buffer with a given size.
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        buffer_pool: &GpuBufferPool,
        size_in_bytes: wgpu::BufferAddress,
        identifier: GpuReadbackIdentifier,
        user_data: GpuReadbackUserDataStorage,
    ) -> GpuReadbackBuffer {
        re_tracing::profile_function!();

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
                re_log::trace!(
                    "Allocating new GpuReadbackBelt chunk of size {:.1} MiB",
                    buffer_size as f32 / (1024.0 * 1024.0)
                );
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
                    last_received_frame_index: u64::MAX,
                }
            }
        };

        let buffer_slice = chunk.allocate(size_in_bytes, identifier, user_data, self.frame_index);
        self.active_chunks.push(chunk);

        buffer_slice
    }

    /// Prepare used buffers for CPU read.
    ///
    /// This should be called after the command encoder(s) used in [`GpuReadbackBuffer`] copy operations are submitted.
    ///
    /// Implementation note:
    /// We can't use [`wgpu::CommandEncoder::map_buffer_on_submit`] here because for that we'd need to know which
    /// command encoder is the last one scheduling any gpu->cpu copy operations.
    /// Note that if chunks were fully tied to a single encoder, we could call [`wgpu::CommandEncoder::map_buffer_on_submit`]
    /// once we know a chunk has all its gpu->cpu copy operations scheduled on that very encoder.
    pub fn after_queue_submit(&mut self) {
        re_tracing::profile_function!();

        // TODO(andreas): Use `map_buffer_on_submit` https://github.com/gfx-rs/wgpu/pull/8125 once available.

        for chunk in self.active_chunks.drain(..) {
            let sender = self.sender.clone();
            chunk.buffer.clone().slice(..chunk.unused_offset).map_async(
                wgpu::MapMode::Read,
                move |result| {
                    if result.is_err() {
                        // This should never happen. Drop the chunk and report.
                        re_log::error_once!("Failed to map staging buffer for reading");
                    } else {
                        sender.send(chunk).ok();
                    }
                },
            );
        }
    }

    /// Should be called at the beginning of a new frame.
    ///
    /// Discards stale data that hasn't been received by
    /// [`GpuReadbackBelt::readback_next_available`]/[`GpuReadbackBelt::readback_newest_available`] for more than a frame.
    pub fn begin_frame(&mut self, frame_index: u64) {
        // Make sure each frame has at least one `receive_chunk` call before it ends (from the pov of the readback belt).
        // It's important to do this before bumping the frame index, because we want to mark all these chunks as "old"
        // chunks that were available for the previous frame.
        // (A user could have done this just before beginning a frame via `receive_chunks` or not call it at all)
        self.receive_chunks();

        self.frame_index = frame_index;

        // Kill off all stale chunks.
        // Note that this happening is unfortunate but not _really_ a user bug as this can happen very easily:
        // For example, if a picking operation is scheduled on view that is immediately closed after!

        // TODO(andreas): just use `Vec::drain_filter` once it goes stable.
        let (discarded, retained) = self.received_chunks.drain(..).partition(|chunk| {
            // If the chunk was received last frame it is too early to discard it, we need to wait one more.
            // Imagine it was received just at the end of that frame - the user has no chance of getting it back
            // at the code that they might be running at the beginning of a frame.
            chunk.last_received_frame_index + 1 < self.frame_index
        });
        self.received_chunks = retained;

        for chunk in discarded {
            re_log::trace!(
                "Unread data from a GpuReadbackBelt was discarded. {} ranges remained unread.",
                chunk.ranges_in_use.len()
            );
            self.reuse_chunk(chunk);
        }
    }

    /// Try to receive a pending data readback with the given identifier and user data type,
    /// calling a callback on the first result that is available.
    ///
    /// *Most likely* subsequent calls will return data from newer submissions. But there is no *strict* guarantee for this.
    /// It could in theory happen that a readback is scheduled after a previous one, but finishes before it!
    /// While there's no documented case of this reordering ever happening, your data handling code should handle this gracefully.
    ///
    /// ATTENTION: Do NOT assume any alignment on the slice passed to `on_data_received`.
    /// See this issue on [Alignment guarantees for mapped buffers](https://github.com/gfx-rs/wgpu/issues/3508).
    ///
    /// Note that [`Self::begin_frame`] will discard stale data automatically, preventing leaks for any
    /// scheduled reads that are never queried.
    // TODO(andreas): Can above mentioned reordering _actually_ happen? How contrived does a setup have to be for this?
    pub fn readback_next_available<UserDataType: 'static, Ret>(
        &mut self,
        identifier: GpuReadbackIdentifier,
        callback: impl FnOnce(&[u8], Box<UserDataType>) -> Ret,
    ) -> Option<Ret> {
        re_tracing::profile_function!();

        self.receive_chunks();
        self.readback_next_available_internal(identifier, callback)
            .map(|(_, result)| result)
    }

    /// Try to receive a pending data readback with the given identifier and user data type,
    /// calling a callback on all results that are available, but returning only the callback result that is associated with
    /// the newest scheduled read.
    ///
    /// *Most likely* subsequent calls will return data from newer submissions. But there is no *strict* guarantee for this.
    /// It could in theory happen that a readback is scheduled after a previous one, but finishes before it!
    /// While there's no documented case of this reordering ever happening, your data handling code should handle this gracefully.
    ///
    /// Note that [`Self::begin_frame`] will discard stale data automatically, preventing leaks for any
    /// scheduled reads that are never queried.
    ///
    /// # Implementation notes
    ///
    /// The callback being called for all results and not just the newest is an artifact of our zero-copy implementation:
    /// Whenever inspecting a result, we hold a pointer directly to the mapped GPU data.
    /// Moving on to the next result without discarding the previous means we'd potentially have two pointers
    /// to the same mapped GPU buffer. This is possible, but inevitably leads to more `unsafe` & more complex code
    /// for something that is expected to be a very rare occurrence.
    //
    // TODO(andreas): Can above mentioned reordering _actually_ happen? How contrived does a setup have to be for this?
    pub fn readback_newest_available<UserDataType: 'static, Ret>(
        &mut self,
        identifier: GpuReadbackIdentifier,
        callback: impl Fn(&[u8], Box<UserDataType>) -> Ret,
    ) -> Option<Ret> {
        re_tracing::profile_function!();

        self.receive_chunks();

        let mut last_result = None;
        let mut last_scheduled_frame_index = 0;

        while let Some((scheduled_frame_index, result)) =
            self.readback_next_available_internal::<UserDataType, Ret>(identifier, &callback)
        {
            // There could be several results from the same frame. Use the most recently received one in that case.
            if scheduled_frame_index >= last_scheduled_frame_index {
                last_result = Some(result);
                last_scheduled_frame_index = scheduled_frame_index;
            }
        }

        last_result
    }

    /// Returns result + frame index for when the request was scheduled.
    fn readback_next_available_internal<UserDataType: 'static, Ret>(
        &mut self,
        identifier: GpuReadbackIdentifier,
        callback: impl FnOnce(&[u8], Box<UserDataType>) -> Ret,
    ) -> Option<(u64, Ret)> {
        // Search for the user data in the readback chunks.
        // A linear search is suited best since we expect both the number of pending chunks (typically just one or two!)
        // as well as the number of readbacks per chunk to be small!
        // Also note that identifiers may not be unique!
        for (chunk_index, chunk) in self.received_chunks.iter_mut().enumerate() {
            for (range_index, range) in chunk.ranges_in_use.iter().enumerate() {
                if range.identifier != identifier || !range.user_data.is::<UserDataType>() {
                    continue;
                }

                let (result, scheduled_frame_index) = {
                    let range = chunk.ranges_in_use.swap_remove(range_index);
                    let slice = chunk.buffer.slice(range.buffer_range.clone());
                    let data = slice.get_mapped_range();
                    (
                        callback(&data, range.user_data.downcast::<UserDataType>().unwrap()),
                        range.scheduled_frame_index,
                    )
                };

                // If this was the last range from this chunk, the chunk is ready for re-use!
                if chunk.ranges_in_use.is_empty() {
                    let chunk = self.received_chunks.swap_remove(chunk_index);
                    self.reuse_chunk(chunk);
                }
                return Some((scheduled_frame_index, result));
            }
        }

        None
    }

    /// Check if any new chunks are ready to be read.
    fn receive_chunks(&mut self) {
        while let Ok(mut chunk) = self.receiver.try_recv() {
            chunk.last_received_frame_index = self.frame_index;
            self.received_chunks.push(chunk);
        }
    }

    fn reuse_chunk(&mut self, mut chunk: Chunk) {
        chunk.buffer.unmap();
        chunk.ranges_in_use.clear();
        chunk.unused_offset = 0;
        self.free_chunks.push(chunk);
    }
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
