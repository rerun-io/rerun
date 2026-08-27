//! Conversion of parsed VPS/SPS/PPS into the `StdVideoH265*` parameter structs
//! that Vulkan video session parameters are built from.
//!
//! The only file in the parser touching `ash` types, and they are inert bindgen
//! structs: no Vulkan calls happen here.

use ash::vk::native as std_video;
use cros_codecs::codec::h265::parser::{Level, Pps, ProfileTierLevel, ScalingLists, Sps, Vps};

/// A `StdVideoH265VideoParameterSet` plus the allocations its pointers refer into.
///
/// The pointers stay valid while this struct is alive, moves included:
/// they point at the boxed parts, which don't move with it.
pub struct VpsStdParams {
    std: std_video::StdVideoH265VideoParameterSet,
    _profile_tier_level: Box<std_video::StdVideoH265ProfileTierLevel>,
    _dec_pic_buf_mgr: Box<std_video::StdVideoH265DecPicBufMgr>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for VpsStdParams {}

impl VpsStdParams {
    pub fn std(&self) -> &std_video::StdVideoH265VideoParameterSet {
        &self.std
    }

    pub fn build(vps: &Vps) -> Self {
        let mut flags = vps_flags();
        flags.set_vps_temporal_id_nesting_flag(vps.temporal_id_nesting_flag.into());
        flags.set_vps_sub_layer_ordering_info_present_flag(
            vps.sub_layer_ordering_info_present_flag.into(),
        );

        let profile_tier_level = Box::new(profile_tier_level(&vps.profile_tier_level));
        let dec_pic_buf_mgr = Box::new(dec_pic_buf_mgr(
            &vps.max_dec_pic_buffering_minus1,
            &vps.max_num_reorder_pics,
        ));

        let std = std_video::StdVideoH265VideoParameterSet {
            flags,
            vps_video_parameter_set_id: vps.video_parameter_set_id,
            vps_max_sub_layers_minus1: vps.max_sub_layers_minus1,
            reserved1: 0,
            reserved2: 0,
            // Timing information only matters for encoding and output timing,
            // neither of which the decoder derives from the VPS.
            vps_num_units_in_tick: 0,
            vps_time_scale: 0,
            vps_num_ticks_poc_diff_one_minus1: 0,
            reserved3: 0,
            pDecPicBufMgr: &raw const *dec_pic_buf_mgr,
            pHrdParameters: std::ptr::null(),
            pProfileTierLevel: &raw const *profile_tier_level,
        };

        Self {
            std,
            _profile_tier_level: profile_tier_level,
            _dec_pic_buf_mgr: dec_pic_buf_mgr,
        }
    }
}

/// A `StdVideoH265SequenceParameterSet` plus the allocations its pointers refer into.
pub struct SpsStdParams {
    std: std_video::StdVideoH265SequenceParameterSet,
    _profile_tier_level: Box<std_video::StdVideoH265ProfileTierLevel>,
    _dec_pic_buf_mgr: Box<std_video::StdVideoH265DecPicBufMgr>,
    _short_term_ref_pic_sets: Vec<std_video::StdVideoH265ShortTermRefPicSet>,
    _long_term_ref_pics: Option<Box<std_video::StdVideoH265LongTermRefPicsSps>>,
    #[expect(dead_code, reason = "keeps the memory `pScalingLists` points at alive")]
    scaling_lists: Option<Box<std_video::StdVideoH265ScalingLists>>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for SpsStdParams {}

impl SpsStdParams {
    pub fn std(&self) -> &std_video::StdVideoH265SequenceParameterSet {
        &self.std
    }

    pub fn build(sps: &Sps) -> Self {
        let mut flags = sps_flags();
        flags.set_sps_temporal_id_nesting_flag(sps.temporal_id_nesting_flag.into());
        flags.set_separate_colour_plane_flag(sps.separate_colour_plane_flag.into());
        flags.set_conformance_window_flag(sps.conformance_window_flag.into());
        flags.set_sps_sub_layer_ordering_info_present_flag(
            sps.sub_layer_ordering_info_present_flag.into(),
        );
        flags.set_scaling_list_enabled_flag(sps.scaling_list_enabled_flag.into());
        flags.set_sps_scaling_list_data_present_flag(sps.scaling_list_data_present_flag.into());
        flags.set_amp_enabled_flag(sps.amp_enabled_flag.into());
        flags.set_sample_adaptive_offset_enabled_flag(
            sps.sample_adaptive_offset_enabled_flag.into(),
        );
        // Pulse code modulation and the extensions are rejected before an SPS gets here.
        flags.set_long_term_ref_pics_present_flag(sps.long_term_ref_pics_present_flag.into());
        flags.set_sps_temporal_mvp_enabled_flag(sps.temporal_mvp_enabled_flag.into());
        flags.set_strong_intra_smoothing_enabled_flag(
            sps.strong_intra_smoothing_enabled_flag.into(),
        );
        // The VUI carries no decode-relevant information, color properties travel
        // through `DecodedFrame` instead.
        flags.set_vui_parameters_present_flag(0);

        let profile_tier_level = Box::new(profile_tier_level(&sps.profile_tier_level));
        let dec_pic_buf_mgr = Box::new(dec_pic_buf_mgr_u8(
            &sps.max_dec_pic_buffering_minus1,
            &sps.max_num_reorder_pics,
            &sps.max_latency_increase_plus1,
        ));

        let scaling_lists = sps
            .scaling_list_data_present_flag
            .then(|| Box::new(scaling_lists(&sps.scaling_list)));

        let short_term_ref_pic_sets: Vec<_> = sps
            .short_term_ref_pic_set
            .iter()
            .map(short_term_ref_pic_set)
            .collect();

        let long_term_ref_pics = sps.long_term_ref_pics_present_flag.then(|| {
            let mut used_by_curr_pic_lt_sps_flag = 0_u32;
            for (index, &used) in sps
                .used_by_curr_pic_lt_sps_flag
                .iter()
                .take(usize::from(sps.num_long_term_ref_pics_sps))
                .enumerate()
            {
                if used {
                    used_by_curr_pic_lt_sps_flag |= 1 << index;
                }
            }
            Box::new(std_video::StdVideoH265LongTermRefPicsSps {
                used_by_curr_pic_lt_sps_flag,
                lt_ref_pic_poc_lsb_sps: sps.lt_ref_pic_poc_lsb_sps,
            })
        });

        let std = std_video::StdVideoH265SequenceParameterSet {
            flags,
            // Only 4:2:0 streams make it past validation.
            chroma_format_idc:
                std_video::StdVideoH265ChromaFormatIdc_STD_VIDEO_H265_CHROMA_FORMAT_IDC_420,
            pic_width_in_luma_samples: u32::from(sps.pic_width_in_luma_samples),
            pic_height_in_luma_samples: u32::from(sps.pic_height_in_luma_samples),
            sps_video_parameter_set_id: sps.video_parameter_set_id,
            sps_max_sub_layers_minus1: sps.max_sub_layers_minus1,
            sps_seq_parameter_set_id: sps.seq_parameter_set_id,
            bit_depth_luma_minus8: sps.bit_depth_luma_minus8,
            bit_depth_chroma_minus8: sps.bit_depth_chroma_minus8,
            log2_max_pic_order_cnt_lsb_minus4: sps.log2_max_pic_order_cnt_lsb_minus4,
            log2_min_luma_coding_block_size_minus3: sps.log2_min_luma_coding_block_size_minus3,
            log2_diff_max_min_luma_coding_block_size: sps.log2_diff_max_min_luma_coding_block_size,
            log2_min_luma_transform_block_size_minus2: sps
                .log2_min_luma_transform_block_size_minus2,
            log2_diff_max_min_luma_transform_block_size: sps
                .log2_diff_max_min_luma_transform_block_size,
            max_transform_hierarchy_depth_inter: sps.max_transform_hierarchy_depth_inter,
            max_transform_hierarchy_depth_intra: sps.max_transform_hierarchy_depth_intra,
            num_short_term_ref_pic_sets: sps.num_short_term_ref_pic_sets,
            num_long_term_ref_pics_sps: sps.num_long_term_ref_pics_sps,
            // Pulse code modulation is rejected, the screen content coding fields
            // with it.
            pcm_sample_bit_depth_luma_minus1: 0,
            pcm_sample_bit_depth_chroma_minus1: 0,
            log2_min_pcm_luma_coding_block_size_minus3: 0,
            log2_diff_max_min_pcm_luma_coding_block_size: 0,
            reserved1: 0,
            reserved2: 0,
            palette_max_size: 0,
            delta_palette_max_predictor_size: 0,
            motion_vector_resolution_control_idc: 0,
            sps_num_palette_predictor_initializers_minus1: 0,
            conf_win_left_offset: sps.conf_win_left_offset,
            conf_win_right_offset: sps.conf_win_right_offset,
            conf_win_top_offset: sps.conf_win_top_offset,
            conf_win_bottom_offset: sps.conf_win_bottom_offset,
            pProfileTierLevel: &raw const *profile_tier_level,
            pDecPicBufMgr: &raw const *dec_pic_buf_mgr,
            pScalingLists: scaling_lists
                .as_deref()
                .map_or(std::ptr::null(), |lists| lists),
            pShortTermRefPicSet: if short_term_ref_pic_sets.is_empty() {
                std::ptr::null()
            } else {
                short_term_ref_pic_sets.as_ptr()
            },
            pLongTermRefPicsSps: long_term_ref_pics
                .as_deref()
                .map_or(std::ptr::null(), |sets| sets),
            pSequenceParameterSetVui: std::ptr::null(),
            pPredictorPaletteEntries: std::ptr::null(),
        };

        Self {
            std,
            _profile_tier_level: profile_tier_level,
            _dec_pic_buf_mgr: dec_pic_buf_mgr,
            _short_term_ref_pic_sets: short_term_ref_pic_sets,
            _long_term_ref_pics: long_term_ref_pics,
            scaling_lists,
        }
    }
}

/// A `StdVideoH265PictureParameterSet` plus the allocations its pointers refer into.
pub struct PpsStdParams {
    std: std_video::StdVideoH265PictureParameterSet,
    _scaling_lists: Option<Box<std_video::StdVideoH265ScalingLists>>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for PpsStdParams {}

impl PpsStdParams {
    pub fn std(&self) -> &std_video::StdVideoH265PictureParameterSet {
        &self.std
    }

    pub fn build(pps: &Pps) -> Self {
        let mut flags = pps_flags();
        flags.set_dependent_slice_segments_enabled_flag(
            pps.dependent_slice_segments_enabled_flag.into(),
        );
        flags.set_output_flag_present_flag(pps.output_flag_present_flag.into());
        flags.set_sign_data_hiding_enabled_flag(pps.sign_data_hiding_enabled_flag.into());
        flags.set_cabac_init_present_flag(pps.cabac_init_present_flag.into());
        flags.set_constrained_intra_pred_flag(pps.constrained_intra_pred_flag.into());
        flags.set_transform_skip_enabled_flag(pps.transform_skip_enabled_flag.into());
        flags.set_cu_qp_delta_enabled_flag(pps.cu_qp_delta_enabled_flag.into());
        flags.set_pps_slice_chroma_qp_offsets_present_flag(
            pps.slice_chroma_qp_offsets_present_flag.into(),
        );
        flags.set_weighted_pred_flag(pps.weighted_pred_flag.into());
        flags.set_weighted_bipred_flag(pps.weighted_bipred_flag.into());
        flags.set_transquant_bypass_enabled_flag(pps.transquant_bypass_enabled_flag.into());
        flags.set_tiles_enabled_flag(pps.tiles_enabled_flag.into());
        flags.set_entropy_coding_sync_enabled_flag(pps.entropy_coding_sync_enabled_flag.into());
        flags.set_uniform_spacing_flag(pps.uniform_spacing_flag.into());
        flags.set_loop_filter_across_tiles_enabled_flag(
            pps.loop_filter_across_tiles_enabled_flag.into(),
        );
        flags.set_pps_loop_filter_across_slices_enabled_flag(
            pps.loop_filter_across_slices_enabled_flag.into(),
        );
        flags.set_deblocking_filter_control_present_flag(
            pps.deblocking_filter_control_present_flag.into(),
        );
        flags.set_deblocking_filter_override_enabled_flag(
            pps.deblocking_filter_override_enabled_flag.into(),
        );
        flags.set_pps_deblocking_filter_disabled_flag(pps.deblocking_filter_disabled_flag.into());
        flags.set_pps_scaling_list_data_present_flag(pps.scaling_list_data_present_flag.into());
        flags.set_lists_modification_present_flag(pps.lists_modification_present_flag.into());
        flags.set_slice_segment_header_extension_present_flag(
            pps.slice_segment_header_extension_present_flag.into(),
        );

        let scaling_lists = pps
            .scaling_list_data_present_flag
            .then(|| Box::new(scaling_lists(&pps.scaling_list)));

        // Tile boundaries are in coding tree block units and fit in 16 bits,
        // the parser bounds them to the picture size.
        let mut column_width_minus1 = [0_u16; 19];
        for (target, &width) in column_width_minus1
            .iter_mut()
            .zip(pps.column_width_minus1.iter())
        {
            *target = width as u16;
        }
        let mut row_height_minus1 = [0_u16; 21];
        for (target, &height) in row_height_minus1
            .iter_mut()
            .zip(pps.row_height_minus1.iter())
        {
            *target = height as u16;
        }

        let std = std_video::StdVideoH265PictureParameterSet {
            flags,
            pps_pic_parameter_set_id: pps.pic_parameter_set_id,
            pps_seq_parameter_set_id: pps.seq_parameter_set_id,
            sps_video_parameter_set_id: pps.sps.video_parameter_set_id,
            num_extra_slice_header_bits: pps.num_extra_slice_header_bits,
            num_ref_idx_l0_default_active_minus1: pps.num_ref_idx_l0_default_active_minus1,
            num_ref_idx_l1_default_active_minus1: pps.num_ref_idx_l1_default_active_minus1,
            init_qp_minus26: pps.init_qp_minus26,
            diff_cu_qp_delta_depth: pps.diff_cu_qp_delta_depth,
            pps_cb_qp_offset: pps.cb_qp_offset,
            pps_cr_qp_offset: pps.cr_qp_offset,
            pps_beta_offset_div2: pps.beta_offset_div2,
            pps_tc_offset_div2: pps.tc_offset_div2,
            log2_parallel_merge_level_minus2: pps.log2_parallel_merge_level_minus2,
            // The range and screen content coding extensions are rejected.
            log2_max_transform_skip_block_size_minus2: 0,
            diff_cu_chroma_qp_offset_depth: 0,
            chroma_qp_offset_list_len_minus1: 0,
            cb_qp_offset_list: [0; 6],
            cr_qp_offset_list: [0; 6],
            log2_sao_offset_scale_luma: 0,
            log2_sao_offset_scale_chroma: 0,
            pps_act_y_qp_offset_plus5: 0,
            pps_act_cb_qp_offset_plus5: 0,
            pps_act_cr_qp_offset_plus3: 0,
            pps_num_palette_predictor_initializers: 0,
            luma_bit_depth_entry_minus8: 0,
            chroma_bit_depth_entry_minus8: 0,
            num_tile_columns_minus1: pps.num_tile_columns_minus1,
            num_tile_rows_minus1: pps.num_tile_rows_minus1,
            reserved1: 0,
            reserved2: 0,
            column_width_minus1,
            row_height_minus1,
            reserved3: 0,
            pScalingLists: scaling_lists
                .as_deref()
                .map_or(std::ptr::null(), |lists| lists),
            pPredictorPaletteEntries: std::ptr::null(),
        };

        Self {
            std,
            _scaling_lists: scaling_lists,
        }
    }
}

fn vps_flags() -> std_video::StdVideoH265VpsFlags {
    std_video::StdVideoH265VpsFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 1]),
        __bindgen_padding_0: [0; 3],
    }
}

fn sps_flags() -> std_video::StdVideoH265SpsFlags {
    std_video::StdVideoH265SpsFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn pps_flags() -> std_video::StdVideoH265PpsFlags {
    std_video::StdVideoH265PpsFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn profile_tier_level(ptl: &ProfileTierLevel) -> std_video::StdVideoH265ProfileTierLevel {
    let mut flags = std_video::StdVideoH265ProfileTierLevelFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 1]),
        __bindgen_padding_0: [0; 3],
    };
    flags.set_general_tier_flag(ptl.general_tier_flag.into());
    flags.set_general_progressive_source_flag(ptl.general_progressive_source_flag.into());
    flags.set_general_interlaced_source_flag(ptl.general_interlaced_source_flag.into());
    flags.set_general_non_packed_constraint_flag(ptl.general_non_packed_constraint_flag.into());
    flags.set_general_frame_only_constraint_flag(ptl.general_frame_only_constraint_flag.into());

    std_video::StdVideoH265ProfileTierLevel {
        flags,
        general_profile_idc: std_profile_idc(ptl.general_profile_idc),
        general_level_idc: std_level_idc(ptl.general_level_idc),
    }
}

fn dec_pic_buf_mgr(
    max_dec_pic_buffering_minus1: &[u32; 7],
    max_num_reorder_pics: &[u32; 7],
) -> std_video::StdVideoH265DecPicBufMgr {
    let mut buffering = [0_u8; 7];
    let mut reorder = [0_u8; 7];
    for index in 0..7 {
        buffering[index] = max_dec_pic_buffering_minus1[index] as u8;
        reorder[index] = max_num_reorder_pics[index] as u8;
    }
    std_video::StdVideoH265DecPicBufMgr {
        max_latency_increase_plus1: [0; 7],
        max_dec_pic_buffering_minus1: buffering,
        max_num_reorder_pics: reorder,
    }
}

fn dec_pic_buf_mgr_u8(
    max_dec_pic_buffering_minus1: &[u8; 7],
    max_num_reorder_pics: &[u8; 7],
    max_latency_increase_plus1: &[u8; 7],
) -> std_video::StdVideoH265DecPicBufMgr {
    std_video::StdVideoH265DecPicBufMgr {
        max_latency_increase_plus1: std::array::from_fn(|index| {
            u32::from(max_latency_increase_plus1[index])
        }),
        max_dec_pic_buffering_minus1: *max_dec_pic_buffering_minus1,
        max_num_reorder_pics: *max_num_reorder_pics,
    }
}

/// The short-term reference picture sets of an SPS, written out as directly
/// signalled sets.
///
/// `cros-codecs` resolves a set that predicts from another one (spec 7-59 to 7-62)
/// into the same flat count deltas a directly signalled set carries, so the
/// prediction flag stays clear here and the deltas are converted back to the
/// syntax elements the driver derives them from. The resulting set describes the
/// same reference pictures either way.
fn short_term_ref_pic_set(
    set: &cros_codecs::codec::h265::parser::ShortTermRefPicSet,
) -> std_video::StdVideoH265ShortTermRefPicSet {
    let flags = std_video::StdVideoH265ShortTermRefPicSetFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 1]),
        __bindgen_padding_0: [0; 3],
    };

    let negative = usize::from(set.num_negative_pics).min(16);
    let positive = usize::from(set.num_positive_pics).min(16);

    // `DeltaPocS0` descends below zero, `DeltaPocS1` ascends above it, both in
    // steps of at least one: the syntax elements are the gaps minus one.
    let mut delta_poc_s0_minus1 = [0_u16; 16];
    let mut previous = 0_i32;
    for (index, target) in delta_poc_s0_minus1.iter_mut().take(negative).enumerate() {
        let delta = set.delta_poc_s0[index];
        *target = (previous - delta - 1).clamp(0, i32::from(u16::MAX)) as u16;
        previous = delta;
    }
    let mut delta_poc_s1_minus1 = [0_u16; 16];
    let mut previous = 0_i32;
    for (index, target) in delta_poc_s1_minus1.iter_mut().take(positive).enumerate() {
        let delta = set.delta_poc_s1[index];
        *target = (delta - previous - 1).clamp(0, i32::from(u16::MAX)) as u16;
        previous = delta;
    }

    let mut used_by_curr_pic_s0_flag = 0_u16;
    for index in 0..negative {
        if set.used_by_curr_pic_s0[index] {
            used_by_curr_pic_s0_flag |= 1 << index;
        }
    }
    let mut used_by_curr_pic_s1_flag = 0_u16;
    for index in 0..positive {
        if set.used_by_curr_pic_s1[index] {
            used_by_curr_pic_s1_flag |= 1 << index;
        }
    }

    std_video::StdVideoH265ShortTermRefPicSet {
        flags,
        delta_idx_minus1: 0,
        use_delta_flag: 0,
        abs_delta_rps_minus1: 0,
        used_by_curr_pic_flag: 0,
        used_by_curr_pic_s0_flag,
        used_by_curr_pic_s1_flag,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
        num_negative_pics: negative as u8,
        num_positive_pics: positive as u8,
        delta_poc_s0_minus1,
        delta_poc_s1_minus1,
    }
}

/// The scaling list values are kept in the scan order the bitstream conveys them in,
/// which is what the `StdVideo` structs expect.
fn scaling_lists(lists: &ScalingLists) -> std_video::StdVideoH265ScalingLists {
    std_video::StdVideoH265ScalingLists {
        ScalingList4x4: lists.scaling_list_4x4,
        ScalingList8x8: lists.scaling_list_8x8,
        ScalingList16x16: lists.scaling_list_16x16,
        // Only the first two of the six 32x32 lists exist in H.265.
        ScalingList32x32: [lists.scaling_list_32x32[0], lists.scaling_list_32x32[1]],
        ScalingListDCCoef16x16: std::array::from_fn(|index| {
            lists.scaling_list_dc_coef_minus8_16x16[index].clamp(0, 255) as u8
        }),
        ScalingListDCCoef32x32: std::array::from_fn(|index| {
            lists.scaling_list_dc_coef_minus8_32x32[index].clamp(0, 255) as u8
        }),
    }
}

/// The `StdVideo` profile an SPS declares, what its session is created against.
pub fn std_profile_idc_of(sps: &Sps) -> std_video::StdVideoH265ProfileIdc {
    std_profile_idc(sps.profile_tier_level.general_profile_idc)
}

fn std_profile_idc(profile_idc: u8) -> std_video::StdVideoH265ProfileIdc {
    match profile_idc {
        1 => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN,
        2 => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN_10,
        3 => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN_STILL_PICTURE,
        4 => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_FORMAT_RANGE_EXTENSIONS,
        9 => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_SCC_EXTENSIONS,
        _ => std_video::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_INVALID,
    }
}

/// Converts a parsed level to the `StdVideo` level enum, which counts levels from 0.
fn std_level_idc(level: Level) -> std_video::StdVideoH265LevelIdc {
    match level {
        Level::L1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_1_0,
        Level::L2 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_2_0,
        Level::L2_1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_2_1,
        Level::L3 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_3_0,
        Level::L3_1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_3_1,
        Level::L4 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_4_0,
        Level::L4_1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_4_1,
        Level::L5 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_0,
        Level::L5_1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_1,
        Level::L5_2 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_5_2,
        Level::L6 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_0,
        Level::L6_1 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_1,
        Level::L6_2 => std_video::StdVideoH265LevelIdc_STD_VIDEO_H265_LEVEL_IDC_6_2,
    }
}
