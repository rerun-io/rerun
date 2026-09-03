//! The decoder orchestration: parser ops in, decoded frames out.
//!
//! [`DecoderCore`] holds everything shared by the two frontends: the parser,
//! parameter-set tracking, the video session, the bitstream upload, and the
//! decode submission. [`TextureDecoder`] copies decoded frames into new NV12
//! images handed to wgpu, [`CpuDecoder`] reads them back into CPU pixel buffers.
//!
//! Sync model: one decode submission and one output-copy submission per frame,
//! ordered by a timeline semaphore. [`TextureDecoder`] keeps up to
//! [`PIPELINE_DEPTH`] frames in flight and only hands a frame to wgpu once the
//! semaphore confirms its copy completed, so the host rarely blocks. The
//! per-frame resources (command pools, bitstream buffer, result-status query)
//! live in slots reused round-robin, and reusing a slot whose work is still
//! running is what blocks. [`CpuDecoder`] waits for every frame. All of this
//! runs on the caller's (decoder worker) thread, never on the render thread.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use ash::vk;
use h264_reader::nal::sps::SeqParameterSet;
use re_video_parsing::ParsedSps;

use crate::{ColorProperties, DecodeError, DecodedFrame, MatrixCoefficients};

use super::Shared;
use super::device::{CommandPool, Device};
use super::dpb::DecodeImages;
use super::h264::{DecodeInfo, DecodeOp, ParseError, Parser, PpsStdParams, SpsStdParams};
use super::output::{OutputImage, OutputPool};
use super::record::{self, CopySource};
use super::session::{SessionParameters, VideoSession, with_profile_list};
use super::sync::TimelineSemaphore;

/// H.264 streams never need more DPB slots than 16 reference frames plus the
/// current one, whatever the hardware would offer.
const MAX_DPB_SLOTS: u32 = 17;

/// How many frames may be in flight on the GPU before decoding blocks the host.
///
/// Sizes the per-frame resource slots, the result-status query pool, and the
/// output-image ring in distinct mode.
const PIPELINE_DEPTH: usize = 4;

/// One decoded frame read back to the CPU, in decode order.
pub struct CpuFrame {
    /// Display size, the coded size minus the SPS frame cropping.
    pub width: u32,
    pub height: u32,

    /// Presentation order key. Frames must be reordered by it (within a group
    /// delimited by IDR frames) before display.
    pub poc: i32,

    pub is_idr: bool,

    /// NV12: the luma plane, followed by the interleaved chroma plane at half
    /// resolution. Rows are tightly packed to the display width.
    pub data: Vec<u8>,
}

struct SpsEntry {
    parsed: Arc<ParsedSps>,
    std: SpsStdParams,
}

/// Everything bound to one video session, recreated together when an SPS
/// changes the coded extent, profile, or DPB slot count.
struct ActiveSession {
    session: VideoSession,
    parameters: Option<SessionParameters>,
    images: DecodeImages,
}

/// One frame's decode work, submitted to the decode queue.
///
/// The output copy must wait for `decode_value` on the core's semaphore.
struct SubmittedDecode {
    decode_value: u64,
    slot_index: usize,
    source: CopySource,
    color: ColorProperties,
}

/// Per-in-flight-frame resources, reused round-robin.
struct FrameSlot {
    decode_pool: CommandPool,
    copy_pool: CommandPool,
    bitstream: Option<super::alloc::Buffer>,

    /// Copy semaphore value of the slot's last use, 0 when never used.
    /// Everything in the slot is free to reuse once it is reached.
    copy_value: u64,
}

/// A submitted decode whose completion has not been observed yet.
struct InFlightDecode {
    copy_value: u64,

    /// The result-status query to read once `copy_value` is reached:
    /// the frame's slot index.
    query_index: u32,
}

/// The session, parsing, and decode submission shared by the two decoder types.
struct DecoderCore {
    shared: Arc<Shared>,
    semaphore: TimelineSemaphore,
    parser: Parser,

    sps: HashMap<u8, SpsEntry>,
    pps: HashMap<u8, PpsStdParams>,

    /// A parameter set changed: the next frame recreates the session parameters.
    parameters_dirty: bool,

    active: Option<ActiveSession>,

    slots: Vec<FrameSlot>,
    next_slot: usize,

    /// Submitted decodes not yet observed complete, in submission order.
    in_flight: VecDeque<InFlightDecode>,

    /// The latest pending copy reading each (image, layer), by semaphore value.
    /// A decode writing that layer, or referencing it in coincide mode (the copy
    /// restores its layout), must wait for the copy on the GPU.
    pending_layer_copies: Vec<(vk::Image, u32, u64)>,
}

impl DecoderCore {
    fn new(shared: Arc<Shared>) -> Result<Self, DecodeError> {
        let decode_family = shared.queue_plan.decode.family_index;
        let copy_family = shared
            .queue_plan
            .copy
            .map_or(decode_family, |copy| copy.family_index);

        let slots = (0..PIPELINE_DEPTH)
            .map(|_| {
                Ok(FrameSlot {
                    decode_pool: CommandPool::new(shared.device.clone(), decode_family)?,
                    copy_pool: CommandPool::new(shared.device.clone(), copy_family)?,
                    bitstream: None,
                    copy_value: 0,
                })
            })
            .collect::<Result<Vec<_>, vk::Result>>()?;

        Ok(Self {
            semaphore: TimelineSemaphore::new(shared.device.clone())?,
            parser: Parser::new(shared.capabilities.max_dpb_slots.min(MAX_DPB_SLOTS) as u8),
            sps: HashMap::new(),
            pps: HashMap::new(),
            parameters_dirty: false,
            active: None,
            slots,
            next_slot: 0,
            in_flight: VecDeque::new(),
            pending_layer_copies: Vec::new(),
            shared,
        })
    }

    /// Parses one annex-b access unit, tracking parameter sets and returning
    /// the frames to decode.
    fn parse(&mut self, data: &[u8]) -> Result<Vec<DecodeInfo>, DecodeError> {
        let mut frames = Vec::new();
        for op in self.parser.push_access_unit(data)? {
            match op {
                DecodeOp::Sps(parsed) => self.activate_sps(parsed)?,

                DecodeOp::Pps(pps) => {
                    self.pps
                        .insert(pps.pic_parameter_set_id.id(), PpsStdParams::build(&pps));
                    self.parameters_dirty = true;
                }

                DecodeOp::DecodeFrame(info) => frames.push(info),

                // Slot deactivation is implicit: a freed slot index is simply
                // re-activated by the next frame decoding into it.
                DecodeOp::FreeSlots(_) => {}
            }
        }
        Ok(frames)
    }

    /// Tracks a new or changed SPS, checking it against the device limits first.
    fn activate_sps(&mut self, parsed: Arc<ParsedSps>) -> Result<(), DecodeError> {
        if let Some(unsupported) =
            crate::h264_unsupported_by_device(&parsed.info, &self.shared.capabilities)
        {
            return Err(DecodeError::ExceedsDeviceLimits(unsupported.to_string()));
        }

        self.sps.insert(
            parsed.sps.seq_parameter_set_id.id(),
            SpsEntry {
                std: SpsStdParams::build(&parsed.sps),
                parsed: parsed.clone(),
            },
        );
        self.parameters_dirty = true;
        self.parser.preset_sps(parsed);

        Ok(())
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    fn reset(&mut self) {
        self.parser.reset();
        // The session and images survive: an IDR re-activates the DPB slots,
        // and a changed SPS recreates them anyway.
    }

    /// Releases in-flight decodes the semaphore already passed: reads their
    /// result-status queries and prunes the pending-copy tracking.
    /// Returns `completed` for the caller to compare frames against.
    fn release_completed(&mut self, completed: u64) -> Result<u64, DecodeError> {
        while let Some(front) = self.in_flight.front() {
            if front.copy_value > completed {
                break;
            }
            let query_index = front.query_index;
            self.in_flight.pop_front();
            self.check_decode_status(query_index)?;
        }
        self.pending_layer_copies
            .retain(|&(_, _, value)| value > completed);
        Ok(completed)
    }

    /// Releases whatever completed so far, without blocking.
    fn poll_completed(&mut self) -> Result<u64, DecodeError> {
        let completed = self.semaphore.completed()?;
        self.release_completed(completed)
    }

    /// Blocks until the semaphore reaches `value`, then releases.
    fn wait_and_release(&mut self, value: u64) -> Result<u64, DecodeError> {
        self.semaphore.wait(value)?;
        // Later submissions may have completed in the meantime.
        let completed = self.semaphore.completed()?.max(value);
        self.release_completed(completed)
    }

    /// Blocks until every submission completed, then releases.
    fn drain(&mut self) -> Result<u64, DecodeError> {
        self.wait_and_release(self.semaphore.last_value())
    }

    /// The next resource slot, waiting out its previous use. This wait is the
    /// backpressure bounding the number of frames in flight.
    fn acquire_slot(&mut self) -> Result<usize, DecodeError> {
        let index = self.next_slot;
        self.next_slot = (index + 1) % self.slots.len();
        let previous_use = self.slots[index].copy_value;
        if previous_use > 0 {
            self.wait_and_release(previous_use)?;
        }
        Ok(index)
    }

    /// Validates the frame against the device limits, (re)creates session state as
    /// needed, uploads the bitstream, and submits the decode on the decode queue.
    fn submit_decode(
        &mut self,
        info: &DecodeInfo,
        data: &[u8],
    ) -> Result<SubmittedDecode, DecodeError> {
        re_tracing::profile_function!();

        let sps_entry = self
            .sps
            .get(&info.sps_id)
            .ok_or(ParseError::MissingReference { what: "SPS" })?;
        let std_profile_idc = sps_entry.std.std().profile_idc;
        let parsed = &sps_entry.parsed.sps;
        let color = color_properties(parsed);

        let [coded_width, coded_height] = sps_entry.parsed.info.coded_extent.map(u32::from);
        let coded_extent = vk::Extent2D {
            width: coded_width,
            height: coded_height,
        };
        let dpb_slots = sps_entry.parsed.info.max_num_ref_frames + 1;

        // The display region: the coded size minus the SPS frame cropping.
        // 4:2:0 progressive crop offsets are in units of two luma samples.
        let (crop_left, crop_right, crop_top, crop_bottom) =
            parsed.frame_cropping.as_ref().map_or((0, 0, 0, 0), |crop| {
                (
                    crop.left_offset * 2,
                    crop.right_offset * 2,
                    crop.top_offset * 2,
                    crop.bottom_offset * 2,
                )
            });
        let display_width = coded_extent
            .width
            .checked_sub(crop_left + crop_right)
            .filter(|&width| width > 0)
            .ok_or(ParseError::Invalid(
                "frame cropping exceeds the coded width",
            ))?;
        let display_height = coded_extent
            .height
            .checked_sub(crop_top + crop_bottom)
            .filter(|&height| height > 0)
            .ok_or(ParseError::Invalid(
                "frame cropping exceeds the coded height",
            ))?;

        self.ensure_session(std_profile_idc, coded_extent, dpb_slots)?;
        self.ensure_parameters()?;
        let slot_index = self.acquire_slot()?;
        let (bitstream_size, slice_offsets) = self.upload_bitstream(slot_index, info, data)?;

        let active = self.active.as_mut().expect("ensured above");
        active.images.select_next_output();

        // Pending copies may still be reading what this decode touches: the
        // output it writes, and in coincide mode its reference layers, whose
        // layout the copy restores. Wait for them on the GPU.
        let write_target = active.images.readback_source(info.activated_slot());
        let wait_value = self
            .pending_layer_copies
            .iter()
            .filter(|&&(image, layer, _)| {
                (image, layer) == write_target
                    || (active.images.coincide
                        && image == active.images.dpb_image()
                        && info
                            .references
                            .iter()
                            .any(|reference| u32::from(reference.slot) == layer))
            })
            .map(|&(_, _, value)| value)
            .max();

        let cmd = self.slots[slot_index].decode_pool.begin()?;
        record::record_decode(
            &self.shared.device,
            cmd,
            &mut active.session,
            active.parameters.as_ref().expect("ensured above"),
            &mut active.images,
            &record::FrameDecode {
                info,
                bitstream_buffer: self.slots[slot_index]
                    .bitstream
                    .as_ref()
                    .expect("uploaded above")
                    .raw,
                bitstream_size,
                slice_offsets: &slice_offsets,
                query_index: slot_index as u32,
            },
        );
        self.slots[slot_index].decode_pool.end()?;
        let decode_value = {
            let queue = self.shared.decode_queue.lock();
            self.semaphore.submit(*queue, cmd, wait_value)?
        };

        let (source_image, source_layer) = write_target;
        Ok(SubmittedDecode {
            decode_value,
            slot_index,
            source: CopySource {
                image: source_image,
                layer: source_layer,
                restore_dpb_layout: active.images.coincide,
                crop_offset: [crop_left.cast_signed(), crop_top.cast_signed()],
                display: [display_width, display_height],
            },
            color,
        })
    }

    /// Records and submits the output copy of a decoded frame on the copy queue,
    /// without waiting. Returns the copy's semaphore value.
    fn submit_copy(
        &mut self,
        decode: &SubmittedDecode,
        record_copy: impl FnOnce(&Device, vk::CommandBuffer),
    ) -> Result<u64, DecodeError> {
        re_tracing::profile_function!();

        let slot = &self.slots[decode.slot_index];
        let cmd = slot.copy_pool.begin()?;
        record_copy(&self.shared.device, cmd);
        slot.copy_pool.end()?;
        let copy_value = {
            let queue = self
                .shared
                .copy_queue
                .as_ref()
                .unwrap_or(&self.shared.decode_queue)
                .lock();
            self.semaphore
                .submit(*queue, cmd, Some(decode.decode_value))?
        };

        self.slots[decode.slot_index].copy_value = copy_value;
        self.in_flight.push_back(InFlightDecode {
            copy_value,
            query_index: decode.slot_index as u32,
        });

        // Track the copy's read of its source layer for later decodes.
        let key = (decode.source.image, decode.source.layer);
        if let Some(entry) = self
            .pending_layer_copies
            .iter_mut()
            .find(|(image, layer, _)| (*image, *layer) == key)
        {
            entry.2 = copy_value;
        } else {
            self.pending_layer_copies.push((key.0, key.1, copy_value));
        }

        Ok(copy_value)
    }

    /// [`Self::submit_copy`], then blocks until the copy completed and the
    /// decode's result status is checked.
    fn submit_copy_and_wait(
        &mut self,
        decode: &SubmittedDecode,
        record_copy: impl FnOnce(&Device, vk::CommandBuffer),
    ) -> Result<(), DecodeError> {
        let copy_value = self.submit_copy(decode, record_copy)?;
        self.wait_and_release(copy_value)?;
        Ok(())
    }

    /// Recreates the session and its images when the SPS demands different ones.
    fn ensure_session(
        &mut self,
        std_profile_idc: vk::native::StdVideoH264ProfileIdc,
        coded_extent: vk::Extent2D,
        dpb_slots: u32,
    ) -> Result<(), DecodeError> {
        let matches = self.active.as_ref().is_some_and(|active| {
            active.session.std_profile_idc == std_profile_idc
                && active.session.coded_extent == coded_extent
                && active.session.max_dpb_slots == dpb_slots
        });
        if matches {
            return Ok(());
        }

        // In-flight decodes reference the session, its query pool, and the images:
        // finish them before dropping anything.
        self.drain()?;
        self.active = None;
        self.pending_layer_copies.clear();

        let queue_plan = &self.shared.queue_plan;
        let decode_family = queue_plan.decode.family_index;
        let copy_family = queue_plan
            .copy
            .map_or(decode_family, |copy| copy.family_index);

        let session = VideoSession::new(
            self.shared.device.clone(),
            &self.shared.video_caps,
            decode_family,
            std_profile_idc,
            coded_extent,
            dpb_slots,
            self.shared
                .capabilities
                .max_active_references
                .min(dpb_slots - 1),
            if queue_plan.decode_supports_result_status {
                PIPELINE_DEPTH as u32
            } else {
                0
            },
        )?;
        let images = DecodeImages::new(
            self.shared.device.clone(),
            std_profile_idc,
            coded_extent,
            dpb_slots,
            self.shared.video_caps.dpb_and_output_coincide,
            PIPELINE_DEPTH as u32,
            decode_family,
            copy_family,
        )?;

        self.active = Some(ActiveSession {
            session,
            parameters: None,
            images,
        });
        self.parameters_dirty = true;
        Ok(())
    }

    /// Recreates the session parameters from all known SPS/PPS when needed.
    fn ensure_parameters(&mut self) -> Result<(), DecodeError> {
        {
            let active = self.active.as_ref().expect("session ensured first");
            if !self.parameters_dirty && active.parameters.is_some() {
                return Ok(());
            }
        }

        // In-flight decodes reference the current parameters object:
        // finish them before dropping it.
        if !self.in_flight.is_empty() {
            self.drain()?;
        }
        let active = self.active.as_mut().expect("session ensured first");

        // The std structs' pointers refer into the entries owned by the maps,
        // which stay alive over the creation call.
        let sps: Vec<_> = self.sps.values().map(|entry| *entry.std.std()).collect();
        let pps: Vec<_> = self.pps.values().map(|entry| *entry.std()).collect();

        active.parameters = None;
        active.parameters = Some(SessionParameters::new(
            self.shared.device.clone(),
            &active.session,
            &sps,
            &pps,
        )?);
        self.parameters_dirty = false;
        Ok(())
    }

    /// Writes the frame's slices into the slot's bitstream buffer, each prefixed
    /// with a 3-byte start code, zero-padded to the device's size alignment.
    ///
    /// Returns the aligned size and the slice offsets within the buffer.
    fn upload_bitstream(
        &mut self,
        slot_index: usize,
        info: &DecodeInfo,
        data: &[u8],
    ) -> Result<(u64, Vec<u32>), DecodeError> {
        re_tracing::profile_function!();

        const START_CODE: [u8; 3] = [0, 0, 1];

        let mut slice_offsets = Vec::with_capacity(info.slice_ranges.len());
        let mut size = 0_u64;
        for range in &info.slice_ranges {
            slice_offsets.push(size as u32);
            size += (START_CODE.len() + range.len()) as u64;
        }
        let alignment = self
            .shared
            .video_caps
            .min_bitstream_buffer_size_alignment
            .max(1);
        let aligned_size = size.next_multiple_of(alignment);

        let slot = &mut self.slots[slot_index];
        if slot
            .bitstream
            .as_ref()
            .is_none_or(|buffer| buffer.size < aligned_size)
        {
            slot.bitstream = None;
            let capacity = aligned_size.next_power_of_two().max(1 << 16);
            let buffer = with_profile_list(
                self.sps
                    .get(&info.sps_id)
                    .expect("checked by the caller")
                    .std
                    .std()
                    .profile_idc,
                |profile_list| {
                    let create_info = vk::BufferCreateInfo::default()
                        .size(capacity)
                        .usage(vk::BufferUsageFlags::VIDEO_DECODE_SRC_KHR)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .push_next(profile_list);
                    super::alloc::Buffer::new_host(self.shared.device.clone(), &create_info)
                },
            )?;
            slot.bitstream = Some(buffer);
        }

        let mapped = slot
            .bitstream
            .as_mut()
            .expect("created above")
            .mapped_slice_mut();
        let mut cursor = 0;
        for range in &info.slice_ranges {
            mapped[cursor..cursor + START_CODE.len()].copy_from_slice(&START_CODE);
            cursor += START_CODE.len();
            mapped[cursor..cursor + range.len()].copy_from_slice(&data[range.clone()]);
            cursor += range.len();
        }
        mapped[cursor..aligned_size as usize].fill(0);

        Ok((aligned_size, slice_offsets))
    }

    /// Reads the result-status query of a completed decode, when supported.
    #[expect(unsafe_code)]
    fn check_decode_status(&self, query_index: u32) -> Result<(), DecodeError> {
        let Some(query_pool) = self
            .active
            .as_ref()
            .and_then(|active| active.session.query_pool)
        else {
            return Ok(());
        };

        // A `VkQueryResultStatusKHR` value: negative means the decode failed.
        let mut status = [0_i32];
        // SAFETY: The query was written by a submission the caller waited on.
        unsafe {
            self.shared.device.raw.get_query_pool_results(
                query_pool,
                query_index,
                &mut status,
                vk::QueryResultFlags::WITH_STATUS_KHR,
            )?;
        }
        if status[0] < 0 {
            return Err(DecodeError::DecodeFailed(status[0]));
        }
        Ok(())
    }
}

impl Drop for DecoderCore {
    fn drop(&mut self) {
        // Frames may still be in flight: never destroy resources the GPU is still using.
        if let Err(err) = self.semaphore.wait_idle() {
            re_log::warn!("Failed to wait for the video decoder to go idle: {err}");
        }
    }
}

/// Color properties from the SPS VUI, missing fields left at their defaults.
fn color_properties(sps: &SeqParameterSet) -> ColorProperties {
    let signal_type = sps
        .vui_parameters
        .as_ref()
        .and_then(|vui| vui.video_signal_type.as_ref());
    ColorProperties {
        full_range: signal_type.is_some_and(|signal| signal.video_full_range_flag),
        matrix_coefficients: match signal_type
            .and_then(|signal| signal.colour_description.as_ref())
            .map(|colour| colour.matrix_coefficients)
        {
            Some(1) => MatrixCoefficients::Bt709,
            Some(5 | 6) => MatrixCoefficients::Bt601,
            _ => MatrixCoefficients::Unspecified,
        },
    }
}

/// Decodes H.264 access units into NV12 `wgpu` textures, in decode order.
///
/// The Vulkan half of [`crate::H264Decoder`], which adds the reordering to
/// presentation order.
pub struct TextureDecoder {
    // Dropped before the pending frames' images: its drop waits for the GPU.
    core: DecoderCore,
    pool: OutputPool,

    /// Frames whose GPU work may still be running, in decode order.
    /// Wrapped for wgpu and emitted once their copy value completed.
    pending: VecDeque<PendingFrame>,
}

/// A frame submitted to the GPU, held back until its output copy completed.
struct PendingFrame {
    copy_value: u64,
    image: OutputImage,
    display: [u32; 2],
    poc: i32,
    pts: i64,
    is_idr: bool,
    color: ColorProperties,
}

impl TextureDecoder {
    pub(super) fn new(shared: Arc<Shared>) -> Result<Self, DecodeError> {
        let pool = OutputPool::new(shared.device.clone(), &shared.queue_plan);
        Ok(Self {
            core: DecoderCore::new(shared)?,
            pool,
            pending: VecDeque::new(),
        })
    }

    /// See [`crate::H264Decoder::preset_sps`].
    pub fn preset_sps(&mut self, sps: Arc<ParsedSps>) -> Result<(), DecodeError> {
        self.core.activate_sps(sps)
    }

    /// Decodes one annex-b access unit, returning finished frames in decode order,
    /// each keyed by its picture order count (the presentation order key within
    /// a group delimited by IDR frames).
    ///
    /// Frames whose GPU work is still running are held back and returned by a
    /// later call (or by [`Self::flush`]), so the returned frames may come from
    /// earlier access units. Blocks only when [`PIPELINE_DEPTH`] frames are
    /// already in flight. Any error leaves the decoder waiting for an IDR frame,
    /// like [`Parser::push_access_unit`].
    pub fn push_access_unit(
        &mut self,
        data: &[u8],
        pts: i64,
    ) -> Result<Vec<(i64, DecodedFrame)>, DecodeError> {
        re_tracing::profile_function!();

        for info in self.core.parse(data)? {
            let decode = self.core.submit_decode(&info, data)?;
            let image = self.pool.acquire(&decode.source)?;
            let copy_value = self.core.submit_copy(&decode, |device, cmd| {
                record::record_output_to_image(
                    device,
                    cmd,
                    &decode.source,
                    image.raw(),
                    image.extent,
                );
            })?;
            self.pending.push_back(PendingFrame {
                copy_value,
                image,
                display: decode.source.display,
                poc: info.poc,
                pts,
                is_idr: info.is_idr,
                color: decode.color,
            });
        }

        let completed = self.core.poll_completed()?;
        Ok(self.take_completed(completed))
    }

    /// Waits for all in-flight GPU work and returns the finished frames.
    ///
    /// Call this once the stream ended.
    pub fn flush(&mut self) -> Result<Vec<(i64, DecodedFrame)>, DecodeError> {
        let completed = self.core.drain()?;
        Ok(self.take_completed(completed))
    }

    /// Wraps and emits the pending frames whose copy completed.
    fn take_completed(&mut self, completed: u64) -> Vec<(i64, DecodedFrame)> {
        let mut frames = Vec::new();
        while self
            .pending
            .front()
            .is_some_and(|frame| frame.copy_value <= completed)
        {
            let frame = self.pending.pop_front().expect("checked above");
            let wrapped = self.pool.wrap(
                frame.image,
                frame.display,
                frame.pts,
                frame.is_idr,
                frame.color,
            );
            frames.push((i64::from(frame.poc), wrapped));
        }
        frames
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    pub fn reset(&mut self) {
        // Finish in-flight work so the pending images are safe to recycle.
        // A decode failure of a stale frame doesn't matter here, only wait errors.
        if let Err(err) = self.core.drain() {
            re_log::warn_once!("Error while draining the video decoder for a seek: {err}");
        }
        for frame in self.pending.drain(..) {
            self.pool.recycle(frame.image);
        }
        self.core.reset();
    }

    /// See [`Parser::reorder_delay`].
    pub fn reorder_delay(&self) -> usize {
        self.core.parser.reorder_delay()
    }

    /// How many frames may be in flight on the GPU, and so held back by
    /// [`Self::push_access_unit`], before decoding blocks on the oldest.
    #[expect(clippy::unused_self)]
    pub fn pipeline_depth(&self) -> usize {
        PIPELINE_DEPTH
    }
}

/// Decodes H.264 access units into CPU pixel buffers.
///
/// Kept around for debugging: a bit-exact readback of what the hardware decoded,
/// for comparison against a software decoder (see `examples/decode_to_yuv.rs`).
pub struct CpuDecoder {
    core: DecoderCore,
    readback: Option<super::alloc::Buffer>,
}

impl CpuDecoder {
    pub(super) fn new(shared: Arc<Shared>) -> Result<Self, DecodeError> {
        Ok(Self {
            core: DecoderCore::new(shared)?,
            readback: None,
        })
    }

    /// Decodes one annex-b access unit, returning its frames in decode order.
    ///
    /// Blocks until the hardware finished. Any error leaves the decoder waiting
    /// for an IDR frame, like [`Parser::push_access_unit`].
    pub fn push_access_unit(&mut self, data: &[u8]) -> Result<Vec<CpuFrame>, DecodeError> {
        re_tracing::profile_function!();

        let mut frames = Vec::new();
        for info in self.core.parse(data)? {
            frames.push(self.decode_frame(&info, data)?);
        }
        Ok(frames)
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    pub fn reset(&mut self) {
        self.core.reset();
    }

    fn decode_frame(&mut self, info: &DecodeInfo, data: &[u8]) -> Result<CpuFrame, DecodeError> {
        re_tracing::profile_function!();

        let decode = self.core.submit_decode(info, data)?;
        let [display_width, display_height] = decode.source.display;

        // The readback buffer: luma plane at 0, chroma plane 4-byte aligned after it.
        let luma_size = u64::from(display_width) * u64::from(display_height);
        let uv_buffer_offset = luma_size.next_multiple_of(4);
        let chroma_size =
            u64::from(display_width.div_ceil(2)) * 2 * u64::from(display_height.div_ceil(2));
        let readback_size = uv_buffer_offset + chroma_size;
        if self
            .readback
            .as_ref()
            .is_none_or(|buffer| buffer.size < readback_size)
        {
            self.readback = None;
            let create_info = vk::BufferCreateInfo::default()
                .size(readback_size)
                .usage(vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            self.readback = Some(super::alloc::Buffer::new_host(
                self.core.shared.device.clone(),
                &create_info,
            )?);
        }

        let readback_buffer = self.readback.as_ref().expect("created above");
        self.core.submit_copy_and_wait(&decode, |device, cmd| {
            record::record_output_to_buffer(
                device,
                cmd,
                &decode.source,
                &record::Readback {
                    buffer: readback_buffer.raw,
                    uv_buffer_offset,
                },
            );
        })?;

        // Repack the two planes tightly, dropping the alignment padding between them.
        let mapped = readback_buffer.mapped_slice();
        let mut nv12 = Vec::with_capacity((luma_size + chroma_size) as usize);
        nv12.extend_from_slice(&mapped[..luma_size as usize]);
        nv12.extend_from_slice(
            &mapped[uv_buffer_offset as usize..(uv_buffer_offset + chroma_size) as usize],
        );

        Ok(CpuFrame {
            width: display_width,
            height: display_height,
            poc: info.poc,
            is_idr: info.is_idr,
            data: nv12,
        })
    }
}
