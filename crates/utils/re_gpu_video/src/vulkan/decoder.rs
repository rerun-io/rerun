//! The decoder orchestration: parser ops in, decoded frames out.
//!
//! v1 sync model: one decode submission and one readback submission per frame,
//! ordered by a timeline semaphore the host then waits on. This runs on the
//! caller's (decoder worker) thread, never on the render thread.

use std::collections::HashMap;
use std::sync::Arc;

use ash::vk;
use h264_reader::nal::sps::SeqParameterSet;

use crate::DecodeError;

use super::Shared;
use super::device::CommandPool;
use super::dpb::DecodeImages;
use super::h264::{DecodeInfo, DecodeOp, ParseError, Parser, PpsStdParams, SpsStdParams};
use super::record;
use super::session::{SessionParameters, VideoSession, with_profile_list};
use super::sync::TimelineSemaphore;

/// H.264 streams never need more DPB slots than 16 reference frames plus the
/// current one, whatever the hardware would offer.
const MAX_DPB_SLOTS: u32 = 17;

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
    parsed: SeqParameterSet,
    std: SpsStdParams,
}

/// Everything bound to one video session, recreated together when an SPS
/// changes the coded extent, profile, or DPB slot count.
struct ActiveSession {
    session: VideoSession,
    parameters: Option<SessionParameters>,
    images: DecodeImages,
}

/// Decodes H.264 access units into CPU pixel buffers.
///
/// The permanent debugging path: bit-exact readback of what the hardware decoded,
/// for comparison against a software decoder (see `examples/decode_to_yuv.rs`).
/// The texture-output decoder of later milestones shares everything but the readback.
pub struct CpuDecoder {
    shared: Arc<Shared>,
    semaphore: TimelineSemaphore,
    parser: Parser,

    sps: HashMap<u8, SpsEntry>,
    pps: HashMap<u8, PpsStdParams>,

    /// A parameter set changed: the next frame recreates the session parameters.
    parameters_dirty: bool,

    active: Option<ActiveSession>,

    bitstream: Option<super::alloc::Buffer>,
    readback: Option<super::alloc::Buffer>,

    decode_pool: CommandPool,
    copy_pool: CommandPool,
}

impl CpuDecoder {
    pub(super) fn new(shared: Arc<Shared>) -> Result<Self, DecodeError> {
        let decode_family = shared.queue_plan.decode.family_index;
        let copy_family = shared
            .queue_plan
            .copy
            .map_or(decode_family, |copy| copy.family_index);

        Ok(Self {
            semaphore: TimelineSemaphore::new(shared.device.clone())?,
            parser: Parser::new(shared.capabilities.max_dpb_slots.min(MAX_DPB_SLOTS) as u8),
            sps: HashMap::new(),
            pps: HashMap::new(),
            parameters_dirty: false,
            active: None,
            bitstream: None,
            readback: None,
            decode_pool: CommandPool::new(shared.device.clone(), decode_family)?,
            copy_pool: CommandPool::new(shared.device.clone(), copy_family)?,
            shared,
        })
    }

    /// Decodes one annex-b access unit, returning its frames in decode order.
    ///
    /// Blocks until the hardware finished. Any error leaves the decoder waiting
    /// for an IDR frame, like [`Parser::push_access_unit`].
    pub fn push_access_unit(&mut self, data: &[u8]) -> Result<Vec<CpuFrame>, DecodeError> {
        re_tracing::profile_function!();

        let mut frames = Vec::new();
        for op in self.parser.push_access_unit(data)? {
            match op {
                DecodeOp::Sps(sps) => {
                    self.sps.insert(
                        sps.seq_parameter_set_id.id(),
                        SpsEntry {
                            std: SpsStdParams::build(&sps),
                            parsed: *sps,
                        },
                    );
                    self.parameters_dirty = true;
                }

                DecodeOp::Pps(pps) => {
                    self.pps
                        .insert(pps.pic_parameter_set_id.id(), PpsStdParams::build(&pps));
                    self.parameters_dirty = true;
                }

                DecodeOp::DecodeFrame(info) => frames.push(self.decode_frame(&info, data)?),

                // Slot deactivation is implicit: a freed slot index is simply
                // re-activated by the next frame decoding into it.
                DecodeOp::FreeSlots(_) => {}
            }
        }
        Ok(frames)
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    pub fn reset(&mut self) {
        self.parser.reset();
        // The session and images survive: an IDR re-activates the DPB slots,
        // and a changed SPS recreates them anyway.
    }

    fn decode_frame(&mut self, info: &DecodeInfo, data: &[u8]) -> Result<CpuFrame, DecodeError> {
        re_tracing::profile_function!();

        let sps_entry = self
            .sps
            .get(&info.sps_id)
            .ok_or(ParseError::MissingReference { what: "SPS" })?;
        let std_profile_idc = sps_entry.std.std().profile_idc;
        let parsed = &sps_entry.parsed;

        let coded_extent = vk::Extent2D {
            width: (parsed.pic_width_in_mbs_minus1 + 1) * 16,
            height: (parsed.pic_height_in_map_units_minus1 + 1) * 16,
        };
        let dpb_slots = parsed.max_num_ref_frames + 1;

        let capabilities = &self.shared.capabilities;
        let [min_width, min_height] = capabilities.min_coded_extent;
        let [max_width, max_height] = capabilities.max_coded_extent;
        if coded_extent.width < min_width
            || coded_extent.height < min_height
            || coded_extent.width > max_width
            || coded_extent.height > max_height
        {
            return Err(DecodeError::ExceedsDeviceLimits(format!(
                "coded size {}x{} is outside the supported range {min_width}x{min_height} to {max_width}x{max_height}",
                coded_extent.width, coded_extent.height,
            )));
        }
        if parsed.max_num_ref_frames > capabilities.max_active_references {
            return Err(DecodeError::ExceedsDeviceLimits(format!(
                "the stream uses up to {} reference frames, the device supports {}",
                parsed.max_num_ref_frames, capabilities.max_active_references,
            )));
        }

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
        let (bitstream_size, slice_offsets) = self.upload_bitstream(info, data)?;

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
                self.shared.device.clone(),
                &create_info,
            )?);
        }

        let active = self.active.as_mut().expect("ensured above");
        let device = &self.shared.device;

        // Decode submission on the decode queue.
        let cmd = self.decode_pool.begin()?;
        record::record_decode(
            device,
            cmd,
            &mut active.session,
            active.parameters.as_ref().expect("ensured above"),
            &mut active.images,
            &record::FrameDecode {
                info,
                bitstream_buffer: self.bitstream.as_ref().expect("uploaded above").raw,
                bitstream_size,
                slice_offsets: &slice_offsets,
            },
        );
        self.decode_pool.end()?;
        let decode_value = {
            let queue = self.shared.decode_queue.lock();
            self.semaphore.submit(*queue, cmd, None)?
        };

        // Readback submission on the copy queue, after the decode.
        let readback_buffer = self.readback.as_ref().expect("created above");
        let cmd = self.copy_pool.begin()?;
        record::record_readback(
            device,
            cmd,
            &active.images,
            info.setup_slot,
            &record::Readback {
                buffer: readback_buffer.raw,
                crop_offset: [crop_left.cast_signed(), crop_top.cast_signed()],
                display: [display_width, display_height],
                uv_buffer_offset,
            },
        );
        self.copy_pool.end()?;
        let copy_value = {
            let queue = self
                .shared
                .copy_queue
                .as_ref()
                .unwrap_or(&self.shared.decode_queue)
                .lock();
            self.semaphore.submit(*queue, cmd, Some(decode_value))?
        };

        self.semaphore.wait(copy_value)?;
        check_decode_status(&self.shared, &active.session)?;

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

        // The per-frame host wait means nothing is in flight, safe to drop.
        self.active = None;

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
            queue_plan.decode_supports_result_status,
        )?;
        let images = DecodeImages::new(
            self.shared.device.clone(),
            std_profile_idc,
            coded_extent,
            dpb_slots,
            self.shared.video_caps.dpb_and_output_coincide,
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
        let active = self.active.as_mut().expect("session ensured first");
        if !self.parameters_dirty && active.parameters.is_some() {
            return Ok(());
        }

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

    /// Writes the frame's slices into the bitstream buffer, each prefixed with a
    /// 3-byte start code, zero-padded to the device's size alignment.
    ///
    /// Returns the aligned size and the slice offsets within the buffer.
    fn upload_bitstream(
        &mut self,
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

        if self
            .bitstream
            .as_ref()
            .is_none_or(|buffer| buffer.size < aligned_size)
        {
            self.bitstream = None;
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
            self.bitstream = Some(buffer);
        }

        let mapped = self
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
}

/// Reads the result-status query of the frame just waited on, when supported.
#[expect(unsafe_code)]
fn check_decode_status(shared: &Shared, session: &VideoSession) -> Result<(), DecodeError> {
    let Some(query_pool) = session.query_pool else {
        return Ok(());
    };

    // A `VkQueryResultStatusKHR` value: negative means the decode failed.
    let mut status = [0_i32];
    // SAFETY: The query was written by the submission the caller waited on.
    unsafe {
        shared.device.raw.get_query_pool_results(
            query_pool,
            0,
            &mut status,
            vk::QueryResultFlags::WITH_STATUS_KHR,
        )?;
    }
    if status[0] < 0 {
        return Err(DecodeError::DecodeFailed(status[0]));
    }
    Ok(())
}

impl Drop for CpuDecoder {
    fn drop(&mut self) {
        // Everything is host-waited per frame, but an error may have left a
        // submission in flight: never destroy resources under the GPU.
        if let Err(err) = self.semaphore.wait_idle() {
            re_log::warn!("Failed to wait for the video decoder to go idle: {err}");
        }
    }
}
