//! How many frames may precede a frame in decoding order but follow it in
//! presentation order, as the stream's parameter sets declare it.
//!
//! Shared by the backends: both emit frames in decoding order, and both hand this
//! to the reorder buffer that puts them back into presentation order.

use cros_codecs::codec::h265::parser::Sps;
use h264_reader::nal::sps::SeqParameterSet;

/// `max_num_reorder_frames` of an SPS: from the VUI when present,
/// otherwise the level-based `MaxDpbFrames` default (spec E.2.1).
pub fn h264(sps: &SeqParameterSet) -> usize {
    if let Some(restrictions) = sps
        .vui_parameters
        .as_ref()
        .and_then(|vui| vui.bitstream_restrictions.as_ref())
    {
        return restrictions.max_num_reorder_frames as usize;
    }

    let profile_idc = u8::from(sps.profile_idc);
    if matches!(profile_idc, 44 | 86 | 100 | 110 | 122 | 244) && sps.constraint_flags.flag3() {
        return 0;
    }

    let frame_size_in_mbs =
        (sps.pic_width_in_mbs_minus1 + 1) * (sps.pic_height_in_map_units_minus1 + 1);
    (max_dpb_mbs(sps) / frame_size_in_mbs.max(1)).min(16) as usize
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

/// `sps_max_num_reorder_pics` of the highest temporal sub-layer, which is the
/// only one a decoder that keeps every picture ever sees.
pub fn h265(sps: &Sps) -> usize {
    usize::from(sps.max_num_reorder_pics[usize::from(sps.max_sub_layers_minus1)])
}
