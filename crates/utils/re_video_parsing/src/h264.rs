//! H.264 bitstream helpers.
//!
//! All "spec X.Y.Z" references are to Rec. ITU-T H.264 (V16, 06/2026):
//! <https://www.itu.int/rec/T-REC-H.264-202606-I>.

use h264_reader::nal::sps::{ChromaFormat, FrameMbsFlags, SeqParameterSet, SpsError};

/// What decoders read out of an H.264 SPS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpsInfo {
    pub profile_idc: u8,

    /// The `constraint_setN_flag` bits, `constraint_set0_flag` first.
    pub constraint_flags: u8,

    /// Level as numbered in the bitstream, e.g. 51 for level 5.1.
    pub level_idc: u8,

    /// Width & height in pixels, with the SPS frame cropping applied.
    pub pixel_dimensions: [u16; 2],

    /// Width & height of the coded picture, rounded up to whole macroblocks.
    ///
    /// This is the size a decoder allocates for, as opposed to [`Self::pixel_dimensions`].
    pub coded_extent: [u16; 2],

    /// `chroma_format_idc`: 0 for monochrome, 1 for 4:2:0, 2 for 4:2:2, 3 for 4:4:4.
    pub chroma_format_idc: u8,

    pub bit_depth_luma: u8,
    pub bit_depth_chroma: u8,

    /// Whether every picture is coded as a full frame, rather than as interlaced fields.
    pub frames_only: bool,

    /// Number of reference frames the stream keeps at most.
    pub max_num_ref_frames: u32,

    /// How many frames may precede a frame in decoding order but follow it in presentation order.
    pub max_num_reorder_frames: u32,
}

impl SpsInfo {
    pub fn new(sps: &SeqParameterSet) -> Result<Self, SpsError> {
        let (width, height) = sps.pixel_dimensions()?;

        Ok(Self {
            profile_idc: u8::from(sps.profile_idc),
            constraint_flags: u8::from(sps.constraint_flags),
            level_idc: sps.level_idc,
            pixel_dimensions: [width as _, height as _],
            coded_extent: [
                ((sps.pic_width_in_mbs_minus1 + 1) * 16) as _,
                ((sps.pic_height_in_map_units_minus1 + 1) * 16) as _,
            ],
            chroma_format_idc: chroma_format_idc(sps.chroma_info.chroma_format),
            bit_depth_luma: sps.chroma_info.bit_depth_luma_minus8 + 8,
            bit_depth_chroma: sps.chroma_info.bit_depth_chroma_minus8 + 8,
            frames_only: matches!(sps.frame_mbs_flags, FrameMbsFlags::Frames),
            max_num_ref_frames: sps.max_num_ref_frames,
            max_num_reorder_frames: max_num_reorder_frames(sps),
        })
    }
}

/// `chroma_format_idc` as it was coded in the bitstream, saturating for values the spec
/// doesn't define.
fn chroma_format_idc(chroma_format: ChromaFormat) -> u8 {
    match chroma_format {
        ChromaFormat::Monochrome => 0,
        ChromaFormat::YUV420 => 1,
        ChromaFormat::YUV422 => 2,
        ChromaFormat::YUV444 => 3,
        ChromaFormat::Invalid(idc) => u8::try_from(idc).unwrap_or(u8::MAX),
    }
}

/// `max_num_reorder_frames` of an SPS: from the VUI when present,
/// otherwise the level-based `MaxDpbFrames` default (spec E.2.1).
pub fn max_num_reorder_frames(sps: &SeqParameterSet) -> u32 {
    if let Some(restrictions) = sps
        .vui_parameters
        .as_ref()
        .and_then(|vui| vui.bitstream_restrictions.as_ref())
    {
        return restrictions.max_num_reorder_frames;
    }

    let profile_idc = u8::from(sps.profile_idc);
    if matches!(profile_idc, 44 | 86 | 100 | 110 | 122 | 244) && sps.constraint_flags.flag3() {
        return 0;
    }

    let frame_size_in_mbs =
        (sps.pic_width_in_mbs_minus1 + 1) * (sps.pic_height_in_map_units_minus1 + 1);
    (max_dpb_mbs(sps) / frame_size_in_mbs.max(1)).min(16)
}

/// `MaxDpbMbs` for the SPS's level (spec Table A-1).
fn max_dpb_mbs(sps: &SeqParameterSet) -> u32 {
    // Level 1b is signaled as level 1.1 plus constraint_set3_flag
    // (or as level_idc 9 where profiles allow).
    if sps.level_idc == 11 && sps.constraint_flags.flag3() {
        return 396;
    }
    match sps.level_idc {
        0..=10 => 396,
        11 => 900,
        12..=20 => 2376,
        21 => 4752,
        22..=30 => 8100,
        31 => 18000,
        32 => 20480,
        33..=41 => 32768,
        42 => 34816,
        43..=50 => 110400,
        51..=52 => 184320,
        // Levels 6 to 6.2, and a permissive default for whatever comes after.
        _ => 696320,
    }
}
