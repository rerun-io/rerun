//! Conversion of parsed SPS/PPS into the `StdVideoH264*` parameter structs that
//! Vulkan video session parameters are built from.
//!
//! The only file in the parser touching `ash` types, and they are inert bindgen
//! structs: no Vulkan calls happen here.

use ash::vk::native as std_video;
use h264_reader::nal::{
    pps::{PicParameterSet, PicScalingMatrix},
    sps::{FrameCropping, PicOrderCntType, ScalingList, SeqParameterSet, SeqScalingMatrix},
};

/// A `StdVideoH264SequenceParameterSet` plus the allocations its pointers refer into.
///
/// The pointers stay valid while this struct is alive, moves included:
/// they point at the boxed/heap parts, which don't move with it.
pub struct SpsStdParams {
    std: std_video::StdVideoH264SequenceParameterSet,
    _offsets_for_ref_frame: Vec<i32>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "keeps the memory `pScalingLists` points at alive")
    )]
    scaling_lists: Option<Box<std_video::StdVideoH264ScalingLists>>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for SpsStdParams {}

impl SpsStdParams {
    pub fn std(&self) -> &std_video::StdVideoH264SequenceParameterSet {
        &self.std
    }

    /// What `pScalingLists` points at, for safe inspection.
    #[cfg(test)]
    pub fn scaling_lists(&self) -> Option<&std_video::StdVideoH264ScalingLists> {
        self.scaling_lists.as_deref()
    }

    pub fn build(sps: &SeqParameterSet) -> Self {
        let mut flags = std_video::StdVideoH264SpsFlags {
            _bitfield_align_1: [],
            _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 2]),
            __bindgen_padding_0: 0,
        };
        flags.set_constraint_set0_flag(sps.constraint_flags.flag0().into());
        flags.set_constraint_set1_flag(sps.constraint_flags.flag1().into());
        flags.set_constraint_set2_flag(sps.constraint_flags.flag2().into());
        flags.set_constraint_set3_flag(sps.constraint_flags.flag3().into());
        flags.set_constraint_set4_flag(sps.constraint_flags.flag4().into());
        flags.set_constraint_set5_flag(sps.constraint_flags.flag5().into());
        flags.set_direct_8x8_inference_flag(sps.direct_8x8_inference_flag.into());
        // Interlaced content is rejected before an SPS gets here.
        flags.set_frame_mbs_only_flag(1);
        flags.set_gaps_in_frame_num_value_allowed_flag(
            sps.gaps_in_frame_num_value_allowed_flag.into(),
        );
        flags.set_qpprime_y_zero_transform_bypass_flag(
            sps.chroma_info.qpprime_y_zero_transform_bypass_flag.into(),
        );
        flags.set_frame_cropping_flag(sps.frame_cropping.is_some().into());
        // The VUI carries no decode-relevant information, color properties travel
        // through `DecodedFrame` instead.
        flags.set_vui_parameters_present_flag(0);

        // POC type specifics, zeroed for the types they don't apply to.
        let mut log2_max_pic_order_cnt_lsb_minus4 = 0;
        let mut delta_pic_order_always_zero = false;
        let mut offset_for_non_ref_pic = 0;
        let mut offset_for_top_to_bottom_field = 0;
        let mut offsets_for_ref_frame = Vec::new();
        let pic_order_cnt_type = match &sps.pic_order_cnt {
            PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: log2,
            } => {
                log2_max_pic_order_cnt_lsb_minus4 = *log2;
                std_video::StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_0
            }
            PicOrderCntType::TypeOne {
                delta_pic_order_always_zero_flag,
                offset_for_non_ref_pic: non_ref,
                offset_for_top_to_bottom_field: top_to_bottom,
                offsets_for_ref_frame: offsets,
            } => {
                delta_pic_order_always_zero = *delta_pic_order_always_zero_flag;
                offset_for_non_ref_pic = *non_ref;
                offset_for_top_to_bottom_field = *top_to_bottom;
                offsets_for_ref_frame = offsets.clone();
                std_video::StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_1
            }
            PicOrderCntType::TypeTwo => std_video::StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_2,
        };
        flags.set_delta_pic_order_always_zero_flag(delta_pic_order_always_zero.into());

        let scaling_lists = sps.chroma_info.scaling_matrix.as_ref().map(|matrix| {
            flags.set_seq_scaling_matrix_present_flag(1);
            Box::new(seq_scaling_lists(matrix))
        });

        let crop = sps.frame_cropping.clone().unwrap_or(FrameCropping {
            left_offset: 0,
            right_offset: 0,
            top_offset: 0,
            bottom_offset: 0,
        });

        let std = std_video::StdVideoH264SequenceParameterSet {
            flags,
            profile_idc: std_profile_idc(u8::from(sps.profile_idc)),
            level_idc: std_level_idc(sps.level_idc),
            // Only 4:2:0 streams make it past validation.
            chroma_format_idc:
                std_video::StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_420,
            seq_parameter_set_id: sps.seq_parameter_set_id.id(),
            bit_depth_luma_minus8: sps.chroma_info.bit_depth_luma_minus8,
            bit_depth_chroma_minus8: sps.chroma_info.bit_depth_chroma_minus8,
            log2_max_frame_num_minus4: sps.log2_max_frame_num_minus4,
            pic_order_cnt_type,
            offset_for_non_ref_pic,
            offset_for_top_to_bottom_field,
            log2_max_pic_order_cnt_lsb_minus4,
            num_ref_frames_in_pic_order_cnt_cycle: offsets_for_ref_frame.len() as u8,
            max_num_ref_frames: sps.max_num_ref_frames as u8,
            reserved1: 0,
            pic_width_in_mbs_minus1: sps.pic_width_in_mbs_minus1,
            pic_height_in_map_units_minus1: sps.pic_height_in_map_units_minus1,
            frame_crop_left_offset: crop.left_offset,
            frame_crop_right_offset: crop.right_offset,
            frame_crop_top_offset: crop.top_offset,
            frame_crop_bottom_offset: crop.bottom_offset,
            reserved2: 0,
            pOffsetForRefFrame: if offsets_for_ref_frame.is_empty() {
                std::ptr::null()
            } else {
                offsets_for_ref_frame.as_ptr()
            },
            pScalingLists: scaling_lists
                .as_deref()
                .map_or(std::ptr::null(), |lists| lists),
            pSequenceParameterSetVui: std::ptr::null(),
        };

        Self {
            std,
            _offsets_for_ref_frame: offsets_for_ref_frame,
            scaling_lists,
        }
    }
}

/// A `StdVideoH264PictureParameterSet` plus the allocations its pointers refer into.
pub struct PpsStdParams {
    std: std_video::StdVideoH264PictureParameterSet,
    _scaling_lists: Option<Box<std_video::StdVideoH264ScalingLists>>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for PpsStdParams {}

impl PpsStdParams {
    pub fn std(&self) -> &std_video::StdVideoH264PictureParameterSet {
        &self.std
    }

    pub fn build(pps: &PicParameterSet) -> Self {
        let mut flags = std_video::StdVideoH264PpsFlags {
            _bitfield_align_1: [],
            _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 1]),
            __bindgen_padding_0: [0; 3],
        };
        flags.set_transform_8x8_mode_flag(
            pps.extension
                .as_ref()
                .is_some_and(|extension| extension.transform_8x8_mode_flag)
                .into(),
        );
        flags.set_redundant_pic_cnt_present_flag(pps.redundant_pic_cnt_present_flag.into());
        flags.set_constrained_intra_pred_flag(pps.constrained_intra_pred_flag.into());
        flags.set_deblocking_filter_control_present_flag(
            pps.deblocking_filter_control_present_flag.into(),
        );
        flags.set_weighted_pred_flag(pps.weighted_pred_flag.into());
        flags.set_bottom_field_pic_order_in_frame_present_flag(
            pps.bottom_field_pic_order_in_frame_present_flag.into(),
        );
        flags.set_entropy_coding_mode_flag(pps.entropy_coding_mode_flag.into());

        let scaling_lists = pps
            .extension
            .as_ref()
            .and_then(|extension| extension.pic_scaling_matrix.as_ref())
            .map(|matrix| {
                flags.set_pic_scaling_matrix_present_flag(1);
                Box::new(pic_scaling_lists(matrix))
            });

        let std = std_video::StdVideoH264PictureParameterSet {
            flags,
            seq_parameter_set_id: pps.seq_parameter_set_id.id(),
            pic_parameter_set_id: pps.pic_parameter_set_id.id(),
            num_ref_idx_l0_default_active_minus1: pps.num_ref_idx_l0_default_active_minus1 as u8,
            num_ref_idx_l1_default_active_minus1: pps.num_ref_idx_l1_default_active_minus1 as u8,
            weighted_bipred_idc: u32::from(pps.weighted_bipred_idc),
            pic_init_qp_minus26: pps.pic_init_qp_minus26 as i8,
            pic_init_qs_minus26: pps.pic_init_qs_minus26 as i8,
            chroma_qp_index_offset: pps.chroma_qp_index_offset as i8,
            // Inferred from `chroma_qp_index_offset` when the extension is absent (spec 7.4.2.2).
            second_chroma_qp_index_offset: pps
                .extension
                .as_ref()
                .map_or(pps.chroma_qp_index_offset, |extension| {
                    extension.second_chroma_qp_index_offset
                }) as i8,
            pScalingLists: scaling_lists
                .as_deref()
                .map_or(std::ptr::null(), |lists| lists),
        };

        Self {
            std,
            _scaling_lists: scaling_lists,
        }
    }
}

fn seq_scaling_lists(matrix: &SeqScalingMatrix) -> std_video::StdVideoH264ScalingLists {
    scaling_lists(&matrix.scaling_list4x4, &matrix.scaling_list8x8)
}

fn pic_scaling_lists(matrix: &PicScalingMatrix) -> std_video::StdVideoH264ScalingLists {
    scaling_lists(
        &matrix.scaling_list4x4,
        matrix.scaling_list8x8.as_deref().unwrap_or(&[]),
    )
}

/// The scaling list values are kept in the scan order the bitstream conveys them in,
/// which is what the `StdVideo` structs expect. Lists that are absent or defaulted are
/// communicated through the masks, the driver applies the spec's fallback rules.
fn scaling_lists(
    lists_4x4: &[ScalingList<16>],
    lists_8x8: &[ScalingList<64>],
) -> std_video::StdVideoH264ScalingLists {
    let mut result = std_video::StdVideoH264ScalingLists {
        scaling_list_present_mask: 0,
        use_default_scaling_matrix_mask: 0,
        ScalingList4x4: [[0; 16]; 6],
        ScalingList8x8: [[0; 64]; 6],
    };

    fn fill<const S: usize>(
        result: &mut std_video::StdVideoH264ScalingLists,
        list: &ScalingList<S>,
        bit: u16,
        target: &mut [u8; S],
    ) {
        match list {
            ScalingList::NotPresent => {}
            ScalingList::UseDefault => {
                result.scaling_list_present_mask |= bit;
                result.use_default_scaling_matrix_mask |= bit;
            }
            ScalingList::List(values) => {
                result.scaling_list_present_mask |= bit;
                for (target, value) in target.iter_mut().zip(values) {
                    *target = value.get();
                }
            }
        }
    }

    // Bits 0..=5 address the 4x4 lists, bits 6..=11 the 8x8 lists.
    for (index, list) in lists_4x4.iter().enumerate().take(6) {
        let mut target = [0; 16];
        fill(&mut result, list, 1 << index, &mut target);
        result.ScalingList4x4[index] = target;
    }
    for (index, list) in lists_8x8.iter().enumerate().take(6) {
        let mut target = [0; 64];
        fill(&mut result, list, 1 << (6 + index), &mut target);
        result.ScalingList8x8[index] = target;
    }

    result
}

fn std_profile_idc(profile_idc: u8) -> std_video::StdVideoH264ProfileIdc {
    match profile_idc {
        66 => std_video::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_BASELINE,
        77 => std_video::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_MAIN,
        100 => std_video::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH,
        _ => std_video::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_INVALID,
    }
}

/// Converts a bitstream `level_idc` (e.g. 51 for level 5.1) to the `StdVideo` level enum,
/// which counts levels from 0. Unknown values round up to the next known level.
fn std_level_idc(level_idc: u8) -> std_video::StdVideoH264LevelIdc {
    // Level 1b (signaled as level_idc 9 where profiles allow) sits between
    // levels 1 and 1.1, and the enum has no entry for it: round up to 1.1.
    if level_idc == 9 {
        return std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_1;
    }

    // The bitstream numbers of the levels the enum counts through,
    // the inverse of `level_idc_number` in `super::super::caps`.
    const LEVELS: [u8; 19] = [
        10, 11, 12, 13, 20, 21, 22, 30, 31, 32, 40, 41, 42, 50, 51, 52, 60, 61, 62,
    ];
    LEVELS
        .iter()
        .position(|&level| level >= level_idc)
        .unwrap_or(LEVELS.len() - 1) as std_video::StdVideoH264LevelIdc
}
