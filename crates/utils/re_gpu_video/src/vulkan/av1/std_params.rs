//! Conversion of the parsed sequence header and frame headers into the
//! `StdVideoAV1*` structs Vulkan session parameters and decode operations take.
//!
//! The only file in the parser touching `ash` types, and they are inert bindgen
//! structs: no Vulkan calls happen here.
//!
//! Unlike H.264 and H.265, AV1 puts almost everything in the per-frame header,
//! so the picture info is a large conversion rebuilt for every picture, while
//! the session parameters carry the sequence header alone.

use ash::vk::native as std_video;
use cros_codecs::codec::av1::parser::{
    CdefParams, ColorConfig, FrameHeaderObu, GlobalMotionParams, LoopFilterParams,
    LoopRestorationParams, QuantizationParams, SUPERRES_DENOM_MIN, SegmentationParams,
    SequenceHeaderObu, TileInfo,
};

/// A `StdVideoAV1SequenceHeader` plus the allocations its pointers refer into.
///
/// The pointers stay valid while this struct is alive, moves included:
/// they point at the boxed parts, which don't move with it.
pub struct SequenceStdParams {
    std: std_video::StdVideoAV1SequenceHeader,
    _color_config: Box<std_video::StdVideoAV1ColorConfig>,
    _timing_info: Option<Box<std_video::StdVideoAV1TimingInfo>>,
}

// SAFETY: The struct's pointers refer into heap allocations it owns,
// nothing is tied to the creating thread.
#[expect(unsafe_code)]
unsafe impl Send for SequenceStdParams {}

impl SequenceStdParams {
    pub fn std(&self) -> &std_video::StdVideoAV1SequenceHeader {
        &self.std
    }

    pub fn build(sequence: &SequenceHeaderObu) -> Self {
        let mut flags = sequence_flags();
        flags.set_still_picture(sequence.still_picture.into());
        flags.set_reduced_still_picture_header(sequence.reduced_still_picture_header.into());
        flags.set_use_128x128_superblock(sequence.use_128x128_superblock.into());
        flags.set_enable_filter_intra(sequence.enable_filter_intra.into());
        flags.set_enable_intra_edge_filter(sequence.enable_intra_edge_filter.into());
        flags.set_enable_interintra_compound(sequence.enable_interintra_compound.into());
        flags.set_enable_masked_compound(sequence.enable_masked_compound.into());
        flags.set_enable_warped_motion(sequence.enable_warped_motion.into());
        flags.set_enable_dual_filter(sequence.enable_dual_filter.into());
        flags.set_enable_order_hint(sequence.enable_order_hint.into());
        flags.set_enable_jnt_comp(sequence.enable_jnt_comp.into());
        flags.set_enable_ref_frame_mvs(sequence.enable_ref_frame_mvs.into());
        flags.set_frame_id_numbers_present_flag(sequence.frame_id_numbers_present_flag.into());
        flags.set_enable_superres(sequence.enable_superres.into());
        flags.set_enable_cdef(sequence.enable_cdef.into());
        flags.set_enable_restoration(sequence.enable_restoration.into());
        flags.set_film_grain_params_present(sequence.film_grain_params_present.into());
        flags.set_timing_info_present_flag(sequence.timing_info_present_flag.into());
        flags.set_initial_display_delay_present_flag(
            sequence.initial_display_delay_present_flag.into(),
        );

        let color_config = Box::new(color_config(&sequence.color_config));
        let timing_info = sequence.timing_info_present_flag.then(|| {
            let mut timing_flags = timing_info_flags();
            timing_flags
                .set_equal_picture_interval(sequence.timing_info.equal_picture_interval.into());
            Box::new(std_video::StdVideoAV1TimingInfo {
                flags: timing_flags,
                num_units_in_display_tick: sequence.timing_info.num_units_in_display_tick,
                time_scale: sequence.timing_info.time_scale,
                num_ticks_per_picture_minus_1: sequence.timing_info.num_ticks_per_picture_minus_1,
            })
        });

        let std = std_video::StdVideoAV1SequenceHeader {
            flags,
            seq_profile: sequence.seq_profile as std_video::StdVideoAV1Profile,
            frame_width_bits_minus_1: sequence.frame_width_bits_minus_1,
            frame_height_bits_minus_1: sequence.frame_height_bits_minus_1,
            max_frame_width_minus_1: sequence.max_frame_width_minus_1,
            max_frame_height_minus_1: sequence.max_frame_height_minus_1,
            delta_frame_id_length_minus_2: sequence.delta_frame_id_length_minus_2 as u8,
            additional_frame_id_length_minus_1: sequence.additional_frame_id_length_minus_1 as u8,
            order_hint_bits_minus_1: sequence.order_hint_bits_minus_1.clamp(0, 255) as u8,
            seq_force_integer_mv: sequence.seq_force_integer_mv as u8,
            seq_force_screen_content_tools: sequence.seq_force_screen_content_tools as u8,
            reserved1: [0; 5],
            pColorConfig: &raw const *color_config,
            pTimingInfo: timing_info.as_deref().map_or(std::ptr::null(), |info| info),
        };

        Self {
            std,
            _color_config: color_config,
            _timing_info: timing_info,
        }
    }
}

/// A `StdVideoDecodeAV1PictureInfo` plus the allocations its pointers refer into.
///
/// Rebuilt for every picture: AV1 signals the loop filter, quantizer,
/// segmentation, tiling, and global motion per frame rather than in a parameter set.
pub struct PictureStdParams {
    std: std_video::StdVideoDecodeAV1PictureInfo,
    _tile_info: Box<std_video::StdVideoAV1TileInfo>,
    _quantization: Box<std_video::StdVideoAV1Quantization>,
    _segmentation: Box<std_video::StdVideoAV1Segmentation>,
    _loop_filter: Box<std_video::StdVideoAV1LoopFilter>,
    _cdef: Box<std_video::StdVideoAV1CDEF>,
    _loop_restoration: Box<std_video::StdVideoAV1LoopRestoration>,
    _global_motion: Box<std_video::StdVideoAV1GlobalMotion>,
    _film_grain: Option<Box<std_video::StdVideoAV1FilmGrain>>,

    // The tile info's four arrays, one entry per tile column or row.
    _mi_col_starts: Vec<u16>,
    _mi_row_starts: Vec<u16>,
    _width_in_sbs_minus_1: Vec<u16>,
    _height_in_sbs_minus_1: Vec<u16>,
}

impl PictureStdParams {
    pub fn std(&self) -> &std_video::StdVideoDecodeAV1PictureInfo {
        &self.std
    }

    pub fn build(header: &FrameHeaderObu) -> Self {
        let mut flags = picture_flags();
        flags.set_error_resilient_mode(header.error_resilient_mode.into());
        flags.set_disable_cdf_update(header.disable_cdf_update.into());
        flags.set_use_superres(header.use_superres.into());
        flags.set_render_and_frame_size_different(header.render_and_frame_size_different.into());
        flags.set_allow_screen_content_tools((header.allow_screen_content_tools != 0).into());
        flags.set_is_filter_switchable(header.is_filter_switchable.into());
        flags.set_force_integer_mv((header.force_integer_mv != 0).into());
        flags.set_frame_size_override_flag(header.frame_size_override_flag.into());
        flags.set_buffer_removal_time_present_flag(header.buffer_removal_time_present_flag.into());
        flags.set_allow_intrabc(header.allow_intrabc.into());
        flags.set_frame_refs_short_signaling(header.frame_refs_short_signaling.into());
        flags.set_allow_high_precision_mv(header.allow_high_precision_mv.into());
        flags.set_is_motion_mode_switchable(header.is_motion_mode_switchable.into());
        flags.set_use_ref_frame_mvs(header.use_ref_frame_mvs.into());
        flags.set_disable_frame_end_update_cdf(header.disable_frame_end_update_cdf.into());
        flags.set_allow_warped_motion(header.allow_warped_motion.into());
        flags.set_reduced_tx_set(header.reduced_tx_set.into());
        flags.set_reference_select(header.reference_select.into());
        flags.set_skip_mode_present(header.skip_mode_present.into());
        flags.set_delta_q_present(header.quantization_params.delta_q_present.into());
        flags.set_delta_lf_present(header.loop_filter_params.delta_lf_present.into());
        flags.set_delta_lf_multi(header.loop_filter_params.delta_lf_multi.into());
        flags.set_segmentation_enabled(header.segmentation_params.segmentation_enabled.into());
        flags
            .set_segmentation_update_map(header.segmentation_params.segmentation_update_map.into());
        flags.set_segmentation_temporal_update(
            header
                .segmentation_params
                .segmentation_temporal_update
                .into(),
        );
        flags.set_segmentation_update_data(
            header.segmentation_params.segmentation_update_data.into(),
        );
        flags.set_UsesLr(header.loop_restoration_params.uses_lr.into());
        flags.set_usesChromaLr(header.loop_restoration_params.uses_chroma_lr.into());
        flags.set_apply_grain(header.film_grain_params.apply_grain.into());

        let tiles = &header.tile_info;
        let tile_cols = usize::try_from(tiles.tile_cols).unwrap_or(0).min(64);
        let tile_rows = usize::try_from(tiles.tile_rows).unwrap_or(0).min(64);
        // The starts arrays hold one entry per tile, the trailing frame edge
        // entry `cros-codecs` keeps is not part of the Vulkan struct.
        let mi_col_starts: Vec<u16> = tiles.mi_col_starts[..tile_cols]
            .iter()
            .map(|&start| start as u16)
            .collect();
        let mi_row_starts: Vec<u16> = tiles.mi_row_starts[..tile_rows]
            .iter()
            .map(|&start| start as u16)
            .collect();
        let width_in_sbs_minus_1: Vec<u16> = tiles.width_in_sbs_minus_1[..tile_cols]
            .iter()
            .map(|&width| width as u16)
            .collect();
        let height_in_sbs_minus_1: Vec<u16> = tiles.height_in_sbs_minus_1[..tile_rows]
            .iter()
            .map(|&height| height as u16)
            .collect();

        let tile_info = Box::new(tile_info(
            tiles,
            &mi_col_starts,
            &mi_row_starts,
            &width_in_sbs_minus_1,
            &height_in_sbs_minus_1,
        ));
        let quantization = Box::new(quantization(&header.quantization_params));
        let segmentation = Box::new(segmentation(&header.segmentation_params));
        let loop_filter = Box::new(loop_filter(&header.loop_filter_params));
        let cdef = Box::new(cdef(&header.cdef_params));
        let loop_restoration = Box::new(loop_restoration(&header.loop_restoration_params));
        let global_motion = Box::new(global_motion(&header.global_motion_params));
        let film_grain = header
            .film_grain_params
            .apply_grain
            .then(|| Box::new(film_grain(header)));

        // `SuperresDenom` is signalled as the offset from its smallest value.
        let coded_denom = header
            .superres_denom
            .saturating_sub(SUPERRES_DENOM_MIN as u32) as u8;

        let std = std_video::StdVideoDecodeAV1PictureInfo {
            flags,
            frame_type: header.frame_type as std_video::StdVideoAV1FrameType,
            current_frame_id: header.current_frame_id,
            OrderHint: header.order_hint as u8,
            primary_ref_frame: header.primary_ref_frame as u8,
            refresh_frame_flags: header.refresh_frame_flags as u8,
            reserved1: 0,
            interpolation_filter: header.interpolation_filter
                as std_video::StdVideoAV1InterpolationFilter,
            TxMode: header.tx_mode as std_video::StdVideoAV1TxMode,
            delta_q_res: header.quantization_params.delta_q_res as u8,
            delta_lf_res: header.loop_filter_params.delta_lf_res,
            SkipModeFrame: [
                header.skip_mode_frame[0] as u8,
                header.skip_mode_frame[1] as u8,
            ],
            coded_denom,
            reserved2: [0; 3],
            OrderHints: std::array::from_fn(|index| header.order_hints[index] as u8),
            // Frame id numbers are rejected before a stream gets here, so no
            // reference frame is ever checked against an expected id.
            expectedFrameId: [0; 8],
            pTileInfo: &raw const *tile_info,
            pQuantization: &raw const *quantization,
            pSegmentation: &raw const *segmentation,
            pLoopFilter: &raw const *loop_filter,
            pCDEF: &raw const *cdef,
            pLoopRestoration: &raw const *loop_restoration,
            pGlobalMotion: &raw const *global_motion,
            pFilmGrain: film_grain
                .as_deref()
                .map_or(std::ptr::null(), |grain| grain),
        };

        Self {
            std,
            _tile_info: tile_info,
            _quantization: quantization,
            _segmentation: segmentation,
            _loop_filter: loop_filter,
            _cdef: cdef,
            _loop_restoration: loop_restoration,
            _global_motion: global_motion,
            _film_grain: film_grain,
            _mi_col_starts: mi_col_starts,
            _mi_row_starts: mi_row_starts,
            _width_in_sbs_minus_1: width_in_sbs_minus_1,
            _height_in_sbs_minus_1: height_in_sbs_minus_1,
        }
    }
}

/// The reference info Vulkan keeps per DPB slot, built from the frame header of
/// the picture the slot holds.
pub fn reference_info(header: &FrameHeaderObu, slot: u8) -> super::ops::ReferenceInfo {
    let mut sign_bias = 0_u8;
    for (name, &biased) in header.ref_frame_sign_bias.iter().enumerate() {
        if biased {
            sign_bias |= 1 << name;
        }
    }

    super::ops::ReferenceInfo {
        slot,
        frame_type: header.frame_type as u8,
        order_hint: header.order_hint as u8,
        saved_order_hints: std::array::from_fn(|index| header.order_hints[index] as u8),
        ref_frame_sign_bias: sign_bias,
        disable_frame_end_update_cdf: header.disable_frame_end_update_cdf,
        segmentation_enabled: header.segmentation_params.segmentation_enabled,
    }
}

/// The `StdVideoDecodeAV1ReferenceInfo` of one DPB slot.
pub fn std_reference_info(
    reference: &super::ops::ReferenceInfo,
) -> std_video::StdVideoDecodeAV1ReferenceInfo {
    let mut flags = reference_flags();
    flags.set_disable_frame_end_update_cdf(reference.disable_frame_end_update_cdf.into());
    flags.set_segmentation_enabled(reference.segmentation_enabled.into());

    std_video::StdVideoDecodeAV1ReferenceInfo {
        flags,
        frame_type: reference.frame_type,
        RefFrameSignBias: reference.ref_frame_sign_bias,
        OrderHint: reference.order_hint,
        SavedOrderHints: reference.saved_order_hints,
    }
}

fn color_config(config: &ColorConfig) -> std_video::StdVideoAV1ColorConfig {
    let mut flags = color_config_flags();
    flags.set_mono_chrome(config.mono_chrome.into());
    flags.set_color_range(config.color_range.into());
    flags.set_separate_uv_delta_q(config.separate_uv_delta_q.into());
    flags.set_color_description_present_flag(config.color_description_present_flag.into());

    std_video::StdVideoAV1ColorConfig {
        flags,
        // Only 8-bit 4:2:0 streams make it past validation.
        BitDepth: 8,
        subsampling_x: 1,
        subsampling_y: 1,
        reserved1: 0,
        color_primaries: config.color_primaries as std_video::StdVideoAV1ColorPrimaries,
        transfer_characteristics: config.transfer_characteristics
            as std_video::StdVideoAV1TransferCharacteristics,
        matrix_coefficients: config.matrix_coefficients as std_video::StdVideoAV1MatrixCoefficients,
        chroma_sample_position: config.chroma_sample_position
            as std_video::StdVideoAV1ChromaSamplePosition,
    }
}

fn tile_info(
    tiles: &TileInfo,
    mi_col_starts: &[u16],
    mi_row_starts: &[u16],
    width_in_sbs_minus_1: &[u16],
    height_in_sbs_minus_1: &[u16],
) -> std_video::StdVideoAV1TileInfo {
    let mut flags = tile_info_flags();
    flags.set_uniform_tile_spacing_flag(tiles.uniform_tile_spacing_flag.into());

    let pointer = |values: &[u16]| {
        if values.is_empty() {
            std::ptr::null()
        } else {
            values.as_ptr()
        }
    };

    std_video::StdVideoAV1TileInfo {
        flags,
        TileCols: tiles.tile_cols as u8,
        TileRows: tiles.tile_rows as u8,
        context_update_tile_id: tiles.context_update_tile_id as u16,
        tile_size_bytes_minus_1: tiles.tile_size_bytes.saturating_sub(1) as u8,
        reserved1: [0; 7],
        pMiColStarts: pointer(mi_col_starts),
        pMiRowStarts: pointer(mi_row_starts),
        pWidthInSbsMinus1: pointer(width_in_sbs_minus_1),
        pHeightInSbsMinus1: pointer(height_in_sbs_minus_1),
    }
}

fn quantization(params: &QuantizationParams) -> std_video::StdVideoAV1Quantization {
    let mut flags = quantization_flags();
    flags.set_using_qmatrix(params.using_qmatrix.into());
    flags.set_diff_uv_delta(params.diff_uv_delta.into());

    std_video::StdVideoAV1Quantization {
        flags,
        base_q_idx: params.base_q_idx as u8,
        DeltaQYDc: params.delta_q_y_dc as i8,
        DeltaQUDc: params.delta_q_u_dc as i8,
        DeltaQUAc: params.delta_q_u_ac as i8,
        DeltaQVDc: params.delta_q_v_dc as i8,
        DeltaQVAc: params.delta_q_v_ac as i8,
        qm_y: params.qm_y as u8,
        qm_u: params.qm_u as u8,
        qm_v: params.qm_v as u8,
    }
}

/// The per-segment features, packed into the bitmask and value table Vulkan takes.
fn segmentation(params: &SegmentationParams) -> std_video::StdVideoAV1Segmentation {
    let mut feature_enabled = [0_u8; 8];
    for (segment, enabled) in feature_enabled.iter_mut().enumerate() {
        for (feature, &is_enabled) in params.feature_enabled[segment].iter().enumerate() {
            if is_enabled {
                *enabled |= 1 << feature;
            }
        }
    }

    std_video::StdVideoAV1Segmentation {
        FeatureEnabled: feature_enabled,
        FeatureData: params.feature_data,
    }
}

fn loop_filter(params: &LoopFilterParams) -> std_video::StdVideoAV1LoopFilter {
    let mut flags = loop_filter_flags();
    flags.set_loop_filter_delta_enabled(params.loop_filter_delta_enabled.into());
    flags.set_loop_filter_delta_update(params.loop_filter_delta_update.into());

    // The parser resolves the deltas against the values inherited from the
    // reference frame, so the arrays are complete either way. The update masks
    // mirror the syntax element they come from.
    let (update_ref_delta, update_mode_delta) = if params.loop_filter_delta_update {
        (0xff, 0x3)
    } else {
        (0, 0)
    };

    std_video::StdVideoAV1LoopFilter {
        flags,
        loop_filter_level: params.loop_filter_level,
        loop_filter_sharpness: params.loop_filter_sharpness,
        update_ref_delta,
        loop_filter_ref_deltas: params.loop_filter_ref_deltas,
        update_mode_delta,
        loop_filter_mode_deltas: params.loop_filter_mode_deltas,
    }
}

fn cdef(params: &CdefParams) -> std_video::StdVideoAV1CDEF {
    let strengths = |values: &[u32; 8]| std::array::from_fn(|index| values[index] as u8);

    std_video::StdVideoAV1CDEF {
        cdef_damping_minus_3: params.cdef_damping.saturating_sub(3) as u8,
        cdef_bits: params.cdef_bits as u8,
        cdef_y_pri_strength: strengths(&params.cdef_y_pri_strength),
        cdef_y_sec_strength: strengths(&params.cdef_y_sec_strength),
        cdef_uv_pri_strength: strengths(&params.cdef_uv_pri_strength),
        cdef_uv_sec_strength: strengths(&params.cdef_uv_sec_strength),
    }
}

fn loop_restoration(params: &LoopRestorationParams) -> std_video::StdVideoAV1LoopRestoration {
    std_video::StdVideoAV1LoopRestoration {
        FrameRestorationType: std::array::from_fn(|plane| {
            params.frame_restoration_type[plane] as std_video::StdVideoAV1FrameRestorationType
        }),
        LoopRestorationSize: params.loop_restoration_size,
    }
}

fn global_motion(params: &GlobalMotionParams) -> std_video::StdVideoAV1GlobalMotion {
    std_video::StdVideoAV1GlobalMotion {
        GmType: std::array::from_fn(|name| params.gm_type[name] as u8),
        gm_params: params.gm_params,
    }
}

fn film_grain(header: &FrameHeaderObu) -> std_video::StdVideoAV1FilmGrain {
    let params = &header.film_grain_params;

    let mut flags = film_grain_flags();
    flags.set_chroma_scaling_from_luma(params.chroma_scaling_from_luma.into());
    flags.set_overlap_flag(params.overlap_flag.into());
    flags.set_clip_to_restricted_range(params.clip_to_restricted_range.into());
    flags.set_update_grain(params.update_grain.into());

    // The scaling points and auto-regressive coefficients are stored in shorter
    // arrays than the parser's upper bounds, which no conforming stream fills.
    let copy = |source: &[u8], target: &mut [u8]| {
        for (target, &value) in target.iter_mut().zip(source) {
            *target = value;
        }
    };
    let coefficients = |source: &[u8], target: &mut [i8]| {
        for (target, &value) in target.iter_mut().zip(source) {
            *target = value.cast_signed();
        }
    };

    let mut point_y_value = [0_u8; 14];
    let mut point_y_scaling = [0_u8; 14];
    copy(&params.point_y_value, &mut point_y_value);
    copy(&params.point_y_scaling, &mut point_y_scaling);
    let mut point_cb_value = [0_u8; 10];
    let mut point_cb_scaling = [0_u8; 10];
    copy(&params.point_cb_value, &mut point_cb_value);
    copy(&params.point_cb_scaling, &mut point_cb_scaling);
    let mut point_cr_value = [0_u8; 10];
    let mut point_cr_scaling = [0_u8; 10];
    copy(&params.point_cr_value, &mut point_cr_value);
    copy(&params.point_cr_scaling, &mut point_cr_scaling);

    let mut ar_coeffs_y_plus_128 = [0_i8; 24];
    let mut ar_coeffs_cb_plus_128 = [0_i8; 25];
    let mut ar_coeffs_cr_plus_128 = [0_i8; 25];
    coefficients(&params.ar_coeffs_y_plus_128, &mut ar_coeffs_y_plus_128);
    coefficients(&params.ar_coeffs_cb_plus_128, &mut ar_coeffs_cb_plus_128);
    coefficients(&params.ar_coeffs_cr_plus_128, &mut ar_coeffs_cr_plus_128);

    std_video::StdVideoAV1FilmGrain {
        flags,
        grain_scaling_minus_8: params.grain_scaling_minus_8,
        ar_coeff_lag: params.ar_coeff_lag as u8,
        ar_coeff_shift_minus_6: params.ar_coeff_shift_minus_6,
        grain_scale_shift: params.grain_scale_shift,
        grain_seed: params.grain_seed,
        film_grain_params_ref_idx: params.film_grain_params_ref_idx,
        num_y_points: params.num_y_points,
        point_y_value,
        point_y_scaling,
        num_cb_points: params.num_cb_points,
        point_cb_value,
        point_cb_scaling,
        num_cr_points: params.num_cr_points,
        point_cr_value,
        point_cr_scaling,
        ar_coeffs_y_plus_128,
        ar_coeffs_cb_plus_128,
        ar_coeffs_cr_plus_128,
        cb_mult: params.cb_mult,
        cb_luma_mult: params.cb_luma_mult,
        cb_offset: params.cb_offset,
        cr_mult: params.cr_mult,
        cr_luma_mult: params.cr_luma_mult,
        cr_offset: params.cr_offset,
    }
}

fn sequence_flags() -> std_video::StdVideoAV1SequenceHeaderFlags {
    std_video::StdVideoAV1SequenceHeaderFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn timing_info_flags() -> std_video::StdVideoAV1TimingInfoFlags {
    std_video::StdVideoAV1TimingInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn color_config_flags() -> std_video::StdVideoAV1ColorConfigFlags {
    std_video::StdVideoAV1ColorConfigFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn picture_flags() -> std_video::StdVideoDecodeAV1PictureInfoFlags {
    std_video::StdVideoDecodeAV1PictureInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn reference_flags() -> std_video::StdVideoDecodeAV1ReferenceInfoFlags {
    std_video::StdVideoDecodeAV1ReferenceInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn tile_info_flags() -> std_video::StdVideoAV1TileInfoFlags {
    std_video::StdVideoAV1TileInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn quantization_flags() -> std_video::StdVideoAV1QuantizationFlags {
    std_video::StdVideoAV1QuantizationFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn loop_filter_flags() -> std_video::StdVideoAV1LoopFilterFlags {
    std_video::StdVideoAV1LoopFilterFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}

fn film_grain_flags() -> std_video::StdVideoAV1FilmGrainFlags {
    std_video::StdVideoAV1FilmGrainFlags {
        _bitfield_align_1: [],
        _bitfield_1: std_video::__BindgenBitfieldUnit::new([0; 4]),
    }
}
