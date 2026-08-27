//! The codec dispatch layer: everything the decoder machinery needs to know
//! about the codec of a stream, behind plain enums.
//!
//! The session, DPB, bitstream, and submission machinery in [`super::decoder`]
//! is codec-neutral and works against these types. Adding a codec means adding
//! a variant to each enum here plus the codec-specific recording arm in
//! [`super::record`], the shared machinery stays untouched.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use ash::vk;
use h264_reader::nal::sps::SeqParameterSet;

use crate::{Codec, ColorProperties, DecodeError, MatrixCoefficients, ParseError};

use super::av1;
use super::caps::VulkanVideoCaps;
use super::device::Device;
use super::h264::{self, DecodeOp, PpsStdParams, SpsStdParams};
use super::h265;
use super::session::{SessionParameters, VideoSession};

/// The video profile all profile-bound Vulkan objects of one session are created
/// against, carrying the codec-specific profile identification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CodecProfile {
    H264 {
        std_profile_idc: vk::native::StdVideoH264ProfileIdc,
    },
    H265 {
        std_profile_idc: vk::native::StdVideoH265ProfileIdc,
    },
    Av1 {
        std_profile: vk::native::StdVideoAV1Profile,

        /// The session may decode frames that apply film grain. Part of the
        /// profile in AV1, so hardware without it needs a separate session.
        film_grain: bool,
    },
}

impl CodecProfile {
    /// Builds the Vulkan profile info (progressive, 4:2:0, 8-bit) and hands it to `f`.
    ///
    /// A closure because the pnext chain borrows stack-owned structs.
    pub fn with_profile<R>(&self, f: impl FnOnce(&vk::VideoProfileInfoKHR<'_>) -> R) -> R {
        let base = vk::VideoProfileInfoKHR::default()
            .chroma_subsampling(vk::VideoChromaSubsamplingFlagsKHR::TYPE_420)
            .luma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8)
            .chroma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8);

        match self {
            Self::H264 { std_profile_idc } => {
                let mut h264_profile = vk::VideoDecodeH264ProfileInfoKHR::default()
                    .std_profile_idc(*std_profile_idc)
                    .picture_layout(vk::VideoDecodeH264PictureLayoutFlagsKHR::PROGRESSIVE);
                let profile = base
                    .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_H264)
                    .push_next(&mut h264_profile);
                f(&profile)
            }

            Self::H265 { std_profile_idc } => {
                let mut h265_profile =
                    vk::VideoDecodeH265ProfileInfoKHR::default().std_profile_idc(*std_profile_idc);
                let profile = base
                    .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_H265)
                    .push_next(&mut h265_profile);
                f(&profile)
            }

            Self::Av1 {
                std_profile,
                film_grain,
            } => {
                let mut av1_profile = vk::VideoDecodeAV1ProfileInfoKHR::default()
                    .std_profile(*std_profile)
                    .film_grain_support(*film_grain);
                let profile = base
                    .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_AV1)
                    .push_next(&mut av1_profile);
                f(&profile)
            }
        }
    }

    /// Like [`Self::with_profile`], wrapped in the `VideoProfileListInfoKHR` that
    /// image and buffer create infos chain.
    pub fn with_profile_list<R>(
        &self,
        f: impl FnOnce(&mut vk::VideoProfileListInfoKHR<'_>) -> R,
    ) -> R {
        self.with_profile(|profile| {
            let mut list =
                vk::VideoProfileListInfoKHR::default().profiles(std::slice::from_ref(profile));
            f(&mut list)
        })
    }
}

/// What becomes of a decoded frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameOutput {
    /// Output the frame as soon as it is decoded.
    Show,

    /// Hold the frame back for a later [`CodecOp::ShowHeldBack`] naming its
    /// [`CodecFrameInfo::picture_id`]. Only AV1 codes frames this way.
    HoldBack,

    /// The frame is decoded as a reference for others and never output.
    None,
}

/// One instruction from [`CodecState::parse`].
pub enum CodecOp {
    /// Decode one frame.
    Decode(CodecFrameInfo),

    /// Output a frame decoded earlier and held back (AV1 `show_existing_frame`).
    ShowHeldBack {
        picture_id: u64,

        /// See [`CodecFrameInfo::evicted_pictures`].
        evicted_pictures: Vec<u64>,
    },
}

/// How a frame's bitstream is laid out in the decode buffer.
pub struct Bitstream<'a> {
    /// Byte ranges in the pushed access unit, uploaded in this order.
    pub ranges: &'a [Range<usize>],

    /// Prefix every range with a 3-byte annex-b start code, which the H.264 and
    /// H.265 slice NALs are stripped of. AV1 OBUs go in verbatim.
    pub start_codes: bool,
}

/// Everything the backend needs to decode one frame, produced by [`CodecState::parse`].
pub enum CodecFrameInfo {
    H264(h264::DecodeInfo),
    H265(h265::DecodeInfo),
    Av1(av1::DecodeInfo),
}

impl CodecFrameInfo {
    /// The DPB slot this frame activates: its own reference slot,
    /// or the scratch slot of a non-reference frame.
    pub fn activated_slot(&self) -> Option<u8> {
        match self {
            Self::H264(info) => info.activated_slot(),
            Self::H265(info) => Some(info.setup_slot),
            Self::Av1(info) => Some(info.setup_slot),
        }
    }

    /// The DPB slots of the reference frames this frame may use.
    pub fn reference_slots(&self) -> impl Iterator<Item = u8> + '_ {
        // One iterator type for all arms, the enum's variants differ in theirs.
        let references: Box<dyn Iterator<Item = u8> + '_> = match self {
            Self::H264(info) => Box::new(info.references.iter().map(|reference| reference.slot)),
            Self::H265(info) => Box::new(info.references.iter().map(|reference| reference.slot)),
            Self::Av1(info) => Box::new(info.references.iter().map(|reference| reference.slot)),
        };
        references
    }

    /// The frame's bitstream within the pushed access unit.
    pub fn bitstream(&self) -> Bitstream<'_> {
        match self {
            Self::H264(info) => Bitstream {
                ranges: &info.slice_ranges,
                start_codes: true,
            },
            Self::H265(info) => Bitstream {
                ranges: &info.slice_ranges,
                start_codes: true,
            },
            Self::Av1(info) => Bitstream {
                ranges: &info.obu_ranges,
                start_codes: false,
            },
        }
    }

    /// The presentation order key within a group delimited by random access
    /// points (picture order count).
    pub fn poc(&self) -> i32 {
        match self {
            Self::H264(info) => info.poc,
            Self::H265(info) => info.poc,
            // AV1 has no picture order count, it states the output of every
            // frame instead. The order hint is the closest thing and only ever
            // reported, never sorted by.
            Self::Av1(info) => info.order_hint,
        }
    }

    /// The frame is a random access point: it starts a new group of pictures,
    /// and the presentation order restarts with it.
    pub fn is_idr(&self) -> bool {
        match self {
            Self::H264(info) => info.is_idr,
            // Not only IDR pictures open a group in H.265, every intra random
            // access point does.
            Self::H265(info) => info.is_irap,
            Self::Av1(info) => info.is_key_frame,
        }
    }

    pub fn output(&self) -> FrameOutput {
        match self {
            Self::H264(_) | Self::H265(_) => FrameOutput::Show,
            Self::Av1(info) => info.output,
        }
    }

    /// Names the frame while it is held back, see [`FrameOutput::HoldBack`].
    pub fn picture_id(&self) -> u64 {
        match self {
            Self::H264(_) | Self::H265(_) => 0,
            Self::Av1(info) => info.picture_id,
        }
    }

    /// Frames held back earlier that can never be output any more. Their held
    /// back output is dropped.
    pub fn evicted_pictures(&self) -> &[u64] {
        match self {
            Self::H264(_) | Self::H265(_) => &[],
            Self::Av1(info) => &info.evicted_pictures,
        }
    }
}

/// Per-frame facts the codec derives from its active parameter sets,
/// consumed by the codec-neutral decode submission.
pub struct FrameFacts {
    pub profile: CodecProfile,

    pub coded_extent: vk::Extent2D,

    /// DPB slot count the stream needs (its reference frames plus the current one).
    pub dpb_slots: u32,

    /// Reference frame count the stream declares, validated against the device limit.
    pub max_ref_frames: u32,

    /// Top-left corner of the display region within the coded image, in luma texels.
    pub crop_offset: [i32; 2],

    /// Display size in luma texels, the coded size minus cropping.
    pub display: [u32; 2],

    pub color: ColorProperties,
}

pub(crate) struct SpsEntry {
    parsed: SeqParameterSet,
    std: SpsStdParams,
}

/// The codec-specific decoder state: the bitstream parser and its parameter sets.
pub enum CodecState {
    H264 {
        parser: h264::Parser,
        sps: HashMap<u8, SpsEntry>,
        pps: HashMap<u8, PpsStdParams>,
    },

    /// The H.265 parser owns its parameter sets, so there is nothing to track
    /// beside it.
    H265 { parser: h265::Parser },

    /// AV1's only parameter set is the sequence header, which the parser owns.
    /// Boxed: the parser keeps the saved state of all eight reference slots.
    Av1 { parser: Box<av1::Parser> },
}

impl CodecState {
    /// `max_dpb_slots` is the device's DPB slot capacity for the codec.
    pub fn new(codec: Codec, max_dpb_slots: u8, video_caps: &VulkanVideoCaps) -> Self {
        match codec {
            Codec::H264 => Self::H264 {
                parser: h264::Parser::new(max_dpb_slots),
                sps: HashMap::new(),
                pps: HashMap::new(),
            },
            Codec::H265 => Self::H265 {
                parser: h265::Parser::new(max_dpb_slots),
            },
            Codec::AV1 => Self::Av1 {
                parser: Box::new(av1::Parser::new(
                    max_dpb_slots,
                    video_caps.av1_film_grain_support,
                )),
            },
        }
    }

    /// AV1 codes the output of every frame, so its frames leave the decoder in
    /// presentation order and never pass through the reorder buffer.
    pub fn emits_in_presentation_order(&self) -> bool {
        matches!(self, Self::Av1 { .. })
    }

    /// Parses one access unit, tracking parameter sets and returning what to
    /// decode and output. Sets `parameters_dirty` when a parameter set changed.
    pub fn parse(
        &mut self,
        data: &[u8],
        parameters_dirty: &mut bool,
    ) -> Result<Vec<CodecOp>, DecodeError> {
        let mut frames = Vec::new();
        match self {
            Self::H264 { parser, sps, pps } => {
                for op in parser.push_access_unit(data)? {
                    match op {
                        DecodeOp::Sps(new_sps) => {
                            sps.insert(
                                new_sps.seq_parameter_set_id.id(),
                                SpsEntry {
                                    std: SpsStdParams::build(&new_sps),
                                    parsed: *new_sps,
                                },
                            );
                            *parameters_dirty = true;
                        }

                        DecodeOp::Pps(new_pps) => {
                            pps.insert(
                                new_pps.pic_parameter_set_id.id(),
                                PpsStdParams::build(&new_pps),
                            );
                            *parameters_dirty = true;
                        }

                        DecodeOp::DecodeFrame(info) => {
                            frames.push(CodecOp::Decode(CodecFrameInfo::H264(info)));
                        }

                        // Slot deactivation is implicit: a freed slot index is simply
                        // re-activated by the next frame decoding into it.
                        DecodeOp::FreeSlots(_) => {}
                    }
                }
            }

            Self::H265 { parser } => {
                for op in parser.push_access_unit(data)? {
                    match op {
                        h265::DecodeOp::ParametersChanged => *parameters_dirty = true,
                        h265::DecodeOp::DecodeFrame(info) => {
                            frames.push(CodecOp::Decode(CodecFrameInfo::H265(info)));
                        }
                    }
                }
            }

            Self::Av1 { parser } => {
                for op in parser.push_access_unit(data)? {
                    match op {
                        av1::DecodeOp::ParametersChanged => *parameters_dirty = true,
                        av1::DecodeOp::DecodeFrame(info) => {
                            frames.push(CodecOp::Decode(CodecFrameInfo::Av1(info)));
                        }
                        av1::DecodeOp::ShowExisting(show) => {
                            frames.push(CodecOp::ShowHeldBack {
                                picture_id: show.picture_id,
                                evicted_pictures: show.evicted_pictures,
                            });
                        }
                    }
                }
            }
        }
        Ok(frames)
    }

    /// Derives the per-frame facts from the frame's active parameter sets.
    pub fn frame_facts(&self, info: &CodecFrameInfo) -> Result<FrameFacts, DecodeError> {
        match (self, info) {
            (Self::H264 { sps, .. }, CodecFrameInfo::H264(info)) => {
                let sps_entry = sps
                    .get(&info.sps_id)
                    .ok_or(h264::ParseError::MissingReference { what: "SPS" })?;
                let parsed = &sps_entry.parsed;

                let coded_extent = vk::Extent2D {
                    width: (parsed.pic_width_in_mbs_minus1 + 1) * 16,
                    height: (parsed.pic_height_in_map_units_minus1 + 1) * 16,
                };

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
                    .ok_or(h264::ParseError::Invalid(
                        "frame cropping exceeds the coded width",
                    ))?;
                let display_height = coded_extent
                    .height
                    .checked_sub(crop_top + crop_bottom)
                    .filter(|&height| height > 0)
                    .ok_or(h264::ParseError::Invalid(
                        "frame cropping exceeds the coded height",
                    ))?;

                Ok(FrameFacts {
                    profile: CodecProfile::H264 {
                        std_profile_idc: sps_entry.std.std().profile_idc,
                    },
                    coded_extent,
                    dpb_slots: parsed.max_num_ref_frames + 1,
                    max_ref_frames: parsed.max_num_ref_frames,
                    crop_offset: [crop_left.cast_signed(), crop_top.cast_signed()],
                    display: [display_width, display_height],
                    color: h264_color_properties(parsed),
                })
            }

            (Self::H265 { parser }, CodecFrameInfo::H265(info)) => {
                let facts = parser.sps_facts(info.sps_id)?;
                let [coded_width, coded_height] = facts.coded_extent;
                Ok(FrameFacts {
                    profile: CodecProfile::H265 {
                        std_profile_idc: facts.std_profile_idc,
                    },
                    coded_extent: vk::Extent2D {
                        width: coded_width,
                        height: coded_height,
                    },
                    dpb_slots: facts.dpb_slots,
                    max_ref_frames: facts.max_ref_frames,
                    crop_offset: facts.crop_offset,
                    display: facts.display,
                    color: facts.color,
                })
            }

            (Self::Av1 { parser }, CodecFrameInfo::Av1(info)) => {
                let facts = parser.seq_facts()?;
                let [coded_width, coded_height] = facts.coded_extent;

                // AV1 codes the display size per frame, and it never exceeds
                // the coded size: a stream asking for more means upscaling on
                // display, which the viewer does itself.
                let display = [
                    info.header.render_width.min(coded_width),
                    info.header.render_height.min(coded_height),
                ];

                Ok(FrameFacts {
                    profile: CodecProfile::Av1 {
                        std_profile: facts.std_profile,
                        film_grain: facts.film_grain_present,
                    },
                    coded_extent: vk::Extent2D {
                        width: coded_width,
                        height: coded_height,
                    },
                    dpb_slots: facts.dpb_slots,
                    max_ref_frames: facts.max_ref_frames,
                    crop_offset: [0, 0],
                    display,
                    color: facts.color,
                })
            }

            _ => Err(DecodeError::Parse(ParseError::Invalid(
                "frame does not belong to the decoder's codec",
            ))),
        }
    }

    /// Creates a session parameters object from all known parameter sets.
    pub fn build_session_parameters(
        &self,
        device: Arc<Device>,
        session: &VideoSession,
    ) -> Result<SessionParameters, vk::Result> {
        match self {
            Self::H264 {
                sps: sps_map,
                pps: pps_map,
                ..
            } => {
                // The std structs' pointers refer into the entries owned by the maps,
                // which stay alive over the creation call.
                let sps: Vec<_> = sps_map.values().map(|entry| *entry.std.std()).collect();
                let pps: Vec<_> = pps_map.values().map(|entry| *entry.std()).collect();
                SessionParameters::new_h264(device, session, &sps, &pps)
            }

            Self::H265 { parser } => {
                // The pointers of the std structs refer into the parser's own
                // allocations, which stay alive over the creation call.
                let (vps, sps, pps) = parser.std_parameter_sets();
                SessionParameters::new_h265(device, session, vps, sps, pps)
            }

            Self::Av1 { parser } => {
                // The pointers of the sequence header refer into the parser's
                // own allocations, which stay alive over the creation call.
                let Ok(sequence) = parser.std_sequence_header() else {
                    // Decoding never starts before a sequence header arrived.
                    return Err(vk::Result::ERROR_INITIALIZATION_FAILED);
                };
                SessionParameters::new_av1(device, session, sequence)
            }
        }
    }

    /// Drops all frame state for a seek. The next access unit must hold a random
    /// access point. Parameter sets are kept.
    pub fn reset(&mut self) {
        match self {
            Self::H264 { parser, .. } => parser.reset(),
            Self::H265 { parser } => parser.reset(),
            Self::Av1 { parser } => parser.reset(),
        }
    }

    /// How many frames may precede a frame in decoding order but follow it in
    /// presentation order.
    pub fn reorder_delay(&self) -> usize {
        match self {
            Self::H264 { parser, .. } => parser.reorder_delay(),
            Self::H265 { parser } => parser.reorder_delay(),
            Self::Av1 { parser } => parser.reorder_delay(),
        }
    }
}

/// Color properties from the H.264 SPS VUI, absent fields left at their defaults.
fn h264_color_properties(sps: &SeqParameterSet) -> ColorProperties {
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
