//! Snapshot tests of the op traces for ffmpeg-generated test assets, plus synthetic tests
//! for the bitstream paths no common encoder produces (POC type 1, wraparounds,
//! long-term references, memory management control operations, `frame_num` gaps).
//!
//! The assets live in `tests/assets/`, see `generate.sh` there. The traces list
//! every [`DecodeOp`] per access unit. Changes to them must be re-reviewed against
//! the spec or a reference decoder.

use std::fmt::Write as _;

use h264_reader::nal::pps::{PicParamSetId, PicParameterSet};
use h264_reader::nal::slice::{
    DecRefPicMarking, FieldPic, MemoryManagementControlOperation, ModificationOfPicNums,
    PicOrderCountLsb, RefPicListModifications, SliceExclusive, SliceFamily, SliceHeader, SliceType,
};
use h264_reader::nal::sps::{
    ChromaInfo, FrameMbsFlags, PicOrderCntType, SeqParamSetId, SeqParameterSet,
};

use super::poc::{PocInput, PocState};
use super::refs::{CurrentFrame, Dpb};
use super::std_params::{PpsStdParams, SpsStdParams};
use super::{DecodeOp, ParseError, Parser, ops::ReferenceInfo};

// --- Snapshot traces over the ffmpeg assets ---

fn asset(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/assets/{name}.h264", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&path).expect("test asset missing, run tests/assets/generate.sh");
    assert!(
        data.len() > 100,
        "Fixture is a stub, git-lfs checkout needed.\nFile path: {path}"
    );
    data
}

/// Splits an elementary stream into access units on the access unit delimiters
/// the assets are generated with.
fn split_on_aud(data: &[u8]) -> Vec<&[u8]> {
    let cuts: Vec<usize> = data
        .windows(4)
        .enumerate()
        .filter_map(|(index, window)| (window == [0, 0, 1, 9]).then_some(index))
        .collect();
    assert!(!cuts.is_empty(), "test asset has no access unit delimiters");

    let mut units = Vec::new();
    for (index, &cut) in cuts.iter().enumerate() {
        let end = cuts.get(index + 1).copied().unwrap_or(data.len());
        units.push(&data[cut..end]);
    }
    units
}

fn trace_asset(name: &str) -> String {
    let mut parser = Parser::new(17);
    let mut trace = String::new();
    for (index, access_unit) in split_on_aud(&asset(name)).iter().enumerate() {
        writeln!(trace, "# AU {index}").unwrap();
        for op in parser.push_access_unit(access_unit).unwrap() {
            writeln!(trace, "{op}").unwrap();
        }
    }
    writeln!(trace, "# reorder delay {}", parser.reorder_delay()).unwrap();
    trace
}

#[test]
fn snapshot_i_only() {
    insta::assert_snapshot!("i_only", trace_asset("i_only"));
}

#[test]
fn snapshot_ippp() {
    insta::assert_snapshot!("ippp", trace_asset("ippp"));
}

#[test]
fn snapshot_ipb() {
    insta::assert_snapshot!("ipb", trace_asset("ipb"));
}

#[test]
fn snapshot_ipb_pyramid() {
    insta::assert_snapshot!("ipb_pyramid", trace_asset("ipb_pyramid"));
}

#[test]
fn snapshot_multi_slice() {
    insta::assert_snapshot!("multi_slice", trace_asset("multi_slice"));
}

#[test]
fn snapshot_sps_change() {
    insta::assert_snapshot!("sps_change", trace_asset("sps_change"));
}

/// Access unit boundaries are detected from the slices themselves, so pushing a whole
/// stream at once must decode identically to pushing it one access unit at a time.
#[test]
fn single_push_matches_per_access_unit_pushes() {
    let data = asset("ipb_pyramid");

    let mut parser = Parser::new(17);
    let mut whole = String::new();
    for op in parser.push_access_unit(&data).unwrap() {
        writeln!(whole, "{op}").unwrap();
    }

    let mut parser = Parser::new(17);
    let mut per_au = String::new();
    for access_unit in split_on_aud(&data) {
        for op in parser.push_access_unit(access_unit).unwrap() {
            // Slice byte ranges are relative to the pushed data and can't be compared
            // across the two runs, but they are not part of the `Display` output.
            writeln!(per_au, "{op}").unwrap();
        }
    }

    assert_eq!(whole, per_au);
}

/// Decoding must start at an IDR frame, so a stream entered in the middle is an error.
#[test]
fn starting_at_a_non_idr_frame_is_an_error() {
    let data = asset("ippp");
    let mut parser = Parser::new(17);
    let result = parser.push_access_unit(split_on_aud(&data)[1]);
    assert!(matches!(result, Err(ParseError::ExpectedIdr)), "{result:?}");
}

/// A reset puts the parser back to waiting for an IDR frame, and the next IDR
/// frame gets it decoding again.
#[test]
fn reset_requires_an_idr_frame() {
    let data = asset("ippp");
    let units = split_on_aud(&data);
    let mut parser = Parser::new(17);
    parser.push_access_unit(units[0]).unwrap();
    parser.push_access_unit(units[1]).unwrap();

    parser.reset();
    let result = parser.push_access_unit(units[2]);
    assert!(matches!(result, Err(ParseError::ExpectedIdr)), "{result:?}");

    // The error keeps the parser waiting, an IDR recovers it.
    let ops = parser.push_access_unit(units[0]).unwrap();
    assert!(ops.iter().any(|op| matches!(
        op,
        DecodeOp::DecodeFrame(info) if info.is_idr && info.sps_id == 0 && info.pps_id == 0
    )));
}

/// A stream needing more DPB slots than the device offers is rejected up front.
#[test]
fn too_small_dpb_is_an_error() {
    let data = asset("ippp");
    // The asset's SPS wants more reference frames than one slot can hold.
    let mut parser = Parser::new(1);
    let result = parser.push_access_unit(split_on_aud(&data)[0]);
    assert!(
        matches!(result, Err(ParseError::TooManyRefFrames { .. })),
        "{result:?}"
    );
}

// --- Synthetic POC tests ---

fn synthetic_sps(pic_order_cnt: PicOrderCntType) -> SeqParameterSet {
    SeqParameterSet {
        profile_idc: 100.into(),
        constraint_flags: 0.into(),
        level_idc: 31,
        seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
        chroma_info: ChromaInfo::default(),
        // MaxFrameNum 16, so wraparounds stay easy to spell out.
        log2_max_frame_num_minus4: 0,
        pic_order_cnt,
        max_num_ref_frames: 2,
        gaps_in_frame_num_value_allowed_flag: false,
        pic_width_in_mbs_minus1: 3,
        pic_height_in_map_units_minus1: 3,
        frame_mbs_flags: FrameMbsFlags::Frames,
        direct_8x8_inference_flag: true,
        frame_cropping: None,
        vui_parameters: None,
    }
}

/// `MaxPicOrderCntLsb` 16.
fn type0_sps() -> SeqParameterSet {
    synthetic_sps(PicOrderCntType::TypeZero {
        log2_max_pic_order_cnt_lsb_minus4: 0,
    })
}

#[expect(
    clippy::fn_params_excessive_bools,
    reason = "mirrors the slice header flags"
)]
fn type0_poc(
    state: &mut PocState,
    sps: &SeqParameterSet,
    is_idr: bool,
    nal_ref_idc: u8,
    lsb: u32,
    has_mmco5: bool,
) -> i32 {
    let lsb = Some(PicOrderCountLsb::Frame(lsb));
    state
        .compute(
            sps,
            &PocInput {
                is_idr,
                nal_ref_idc,
                frame_num: 0, // Unused by type 0.
                pic_order_cnt_lsb: &lsb,
                has_mmco5,
            },
        )
        .unwrap()
        .poc()
}

/// The MSB keeps counting across `pic_order_cnt_lsb` wraparounds (spec 8.2.1.1).
#[test]
fn poc_type0_wraparound() {
    let sps = type0_sps();
    let mut state = PocState::default();

    assert_eq!(type0_poc(&mut state, &sps, true, 3, 0, false), 0);
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 4, false), 4);
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 12, false), 12);
    // Forward wrap: 12 -> 2 differs by more than MaxPicOrderCntLsb / 2.
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 2, false), 18);
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 10, false), 26);
    // A non-reference frame presented before the previous reference frame,
    // wrapping backward below the current MSB.
    assert_eq!(type0_poc(&mut state, &sps, false, 0, 4, false), 20);
    // Non-reference frames don't update the prediction state.
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 12, false), 28);
}

/// Memory management control operation 5 rebases the current frame's POC to zero
/// and later frames continue from there (spec 8.2.1).
#[test]
fn poc_type0_mmco5_rebase() {
    let sps = type0_sps();
    let mut state = PocState::default();

    assert_eq!(type0_poc(&mut state, &sps, true, 3, 0, false), 0);
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 8, true), 0);
    // prevPicOrderCntLsb is now the rebased 0, so lsb 4 counts from there.
    assert_eq!(type0_poc(&mut state, &sps, false, 3, 4, false), 4);
}

fn type1_poc(
    state: &mut PocState,
    sps: &SeqParameterSet,
    is_idr: bool,
    nal_ref_idc: u8,
    frame_num: u16,
) -> i32 {
    let lsb = Some(PicOrderCountLsb::FieldsDelta([0, 0]));
    state
        .compute(
            sps,
            &PocInput {
                is_idr,
                nal_ref_idc,
                frame_num,
                pic_order_cnt_lsb: &lsb,
                has_mmco5: false,
            },
        )
        .unwrap()
        .poc()
}

/// POC type 1 walks the expected-delta cycle from the SPS, with the non-reference
/// offset applied on top and `FrameNumOffset` carrying over wraparounds (spec 8.2.1.2).
#[test]
fn poc_type1_cycle_and_wraparound() {
    let sps = synthetic_sps(PicOrderCntType::TypeOne {
        delta_pic_order_always_zero_flag: true,
        offset_for_non_ref_pic: -1,
        offset_for_top_to_bottom_field: 0,
        offsets_for_ref_frame: vec![2],
    });
    let mut state = PocState::default();

    assert_eq!(type1_poc(&mut state, &sps, true, 3, 0), 0);
    assert_eq!(type1_poc(&mut state, &sps, false, 3, 1), 2);
    assert_eq!(type1_poc(&mut state, &sps, false, 3, 2), 4);
    // Non-reference frame: absFrameNum decrements by one, then the offset applies.
    assert_eq!(type1_poc(&mut state, &sps, false, 0, 3), 3);
    assert_eq!(type1_poc(&mut state, &sps, false, 3, 3), 6);

    // frame_num wraparound: MaxFrameNum 16 accumulates into FrameNumOffset.
    for frame_num in 4..16 {
        type1_poc(&mut state, &sps, false, 3, frame_num);
    }
    assert_eq!(type1_poc(&mut state, &sps, false, 3, 0), 32);
    assert_eq!(type1_poc(&mut state, &sps, false, 3, 1), 34);
}

/// POC type 2 mirrors decoding order, with non-reference frames placed just
/// before the reference frame sharing their `frame_num` (spec 8.2.1.3).
#[test]
fn poc_type2() {
    let sps = synthetic_sps(PicOrderCntType::TypeTwo);
    let mut state = PocState::default();

    let poc = |state: &mut PocState, is_idr, nal_ref_idc, frame_num| {
        state
            .compute(
                &sps,
                &PocInput {
                    is_idr,
                    nal_ref_idc,
                    frame_num,
                    pic_order_cnt_lsb: &None,
                    has_mmco5: false,
                },
            )
            .unwrap()
            .poc()
    };

    assert_eq!(poc(&mut state, true, 3, 0), 0);
    assert_eq!(poc(&mut state, false, 3, 1), 2);
    assert_eq!(poc(&mut state, false, 0, 2), 3);
    assert_eq!(poc(&mut state, false, 3, 2), 4);

    // frame_num wraparound.
    for frame_num in 3..16 {
        poc(&mut state, false, 3, frame_num);
    }
    assert_eq!(poc(&mut state, false, 3, 0), 32);
}

// --- Synthetic DPB and reference list tests ---

fn synthetic_pps() -> PicParameterSet {
    PicParameterSet {
        pic_parameter_set_id: PicParamSetId::from_u32(0).unwrap(),
        seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
        entropy_coding_mode_flag: true,
        bottom_field_pic_order_in_frame_present_flag: false,
        slice_groups: None,
        num_ref_idx_l0_default_active_minus1: 3,
        num_ref_idx_l1_default_active_minus1: 3,
        weighted_pred_flag: false,
        weighted_bipred_idc: 0,
        pic_init_qp_minus26: 0,
        pic_init_qs_minus26: 0,
        chroma_qp_index_offset: 0,
        deblocking_filter_control_present_flag: false,
        constrained_intra_pred_flag: false,
        redundant_pic_cnt_present_flag: false,
        extension: None,
    }
}

fn p_slice_header(
    frame_num: u16,
    modifications: Vec<ModificationOfPicNums>,
    marking: DecRefPicMarking,
) -> SliceHeader {
    SliceHeader {
        first_mb_in_slice: 0,
        slice_type: SliceType {
            family: SliceFamily::P,
            exclusive: SliceExclusive::NonExclusive,
        },
        colour_plane: None,
        frame_num,
        field_pic: FieldPic::Frame,
        idr_pic_id: None,
        pic_order_cnt_lsb: Some(PicOrderCountLsb::Frame(0)),
        redundant_pic_cnt: None,
        direct_spatial_mv_pred_flag: None,
        num_ref_idx_active: None,
        ref_pic_list_modification: Some(RefPicListModifications::P {
            ref_pic_list_modification_l0: modifications,
        }),
        pred_weight_table: None,
        dec_ref_pic_marking: Some(marking),
        cabac_init_idc: None,
        slice_qp_delta: 0,
        sp_for_switch_flag: None,
        slice_qs: None,
        disable_deblocking_filter_idc: 0,
    }
}

fn current(frame_num: u16, poc: i32) -> CurrentFrame {
    CurrentFrame {
        frame_num,
        poc: super::poc::Poc {
            top: poc,
            bottom: poc,
        },
        max_frame_num: 16,
    }
}

/// A DPB with `max_num_ref_frames` 2 holding an IDR (slot 0) and a P frame (slot 1).
fn dpb_with_two_refs() -> Dpb {
    let mut dpb = Dpb::default();
    dpb.configure(&type0_sps(), 17).unwrap();

    let outcome = dpb
        .mark(
            &current(0, 0),
            Some(&DecRefPicMarking::Idr {
                no_output_of_prior_pics_flag: false,
                long_term_reference_flag: false,
            }),
        )
        .unwrap();
    assert_eq!(outcome.setup_slot, Some(0));

    let outcome = dpb
        .mark(&current(1, 2), Some(&DecRefPicMarking::SlidingWindow))
        .unwrap();
    assert_eq!(outcome.setup_slot, Some(1));

    dpb
}

/// The sliding window evicts the short-term reference with the smallest
/// `FrameNumWrap`, including across `frame_num` wraparounds (spec 8.2.5.3).
#[test]
fn sliding_window_eviction() {
    let mut dpb = dpb_with_two_refs();

    // At capacity: the IDR frame (frame_num 0) is the oldest and goes first.
    let outcome = dpb
        .mark(&current(2, 4), Some(&DecRefPicMarking::SlidingWindow))
        .unwrap();
    assert_eq!(outcome.setup_slot, Some(2));
    assert_eq!(outcome.freed, vec![0]);

    // Wraparound: frame_num 15 then 0. FrameNumWrap makes 15 older than 0.
    for frame_num in [3, 15] {
        dpb.mark(
            &current(frame_num, 0),
            Some(&DecRefPicMarking::SlidingWindow),
        )
        .unwrap();
    }
    let outcome = dpb
        .mark(&current(0, 0), Some(&DecRefPicMarking::SlidingWindow))
        .unwrap();
    assert_eq!(
        outcome.freed.len(),
        1,
        "sliding window evicts exactly one frame"
    );
}

/// Non-reference frames get no DPB slot and evict nothing.
#[test]
fn non_reference_frames_stay_out_of_the_dpb() {
    let mut dpb = dpb_with_two_refs();
    let outcome = dpb.mark(&current(2, 4), None).unwrap();
    assert_eq!(outcome.setup_slot, None);
    assert!(outcome.freed.is_empty());
}

/// P list initialization: short-term by descending `PicNum`,
/// then long-term by ascending index (spec 8.2.4.2.1).
#[test]
fn p_list_initialization_order() {
    let dpb = dpb_with_two_refs();
    let header = p_slice_header(2, vec![], DecRefPicMarking::SlidingWindow);

    let mut references = Vec::new();
    let lists = dpb
        .ref_lists(
            &type0_sps(),
            &synthetic_pps(),
            &header,
            &current(2, 4),
            &mut references,
        )
        .unwrap();

    // frame_num 1 has the higher PicNum and comes first.
    assert_eq!(lists.l0, vec![1, 0]);
    assert!(lists.l1.is_empty());
    assert_eq!(
        references.iter().map(|r| r.slot).collect::<Vec<_>>(),
        vec![1, 0]
    );
}

/// A `Subtract` modification moves the addressed frame to the front of the list
/// without duplicating it (spec 8.2.4.3.1).
#[test]
fn ref_list_modification_reorders() {
    let dpb = dpb_with_two_refs();
    // abs_diff_pic_num_minus1 = 1: picNum 2 - 2 = 0, the IDR frame.
    let header = p_slice_header(
        2,
        vec![ModificationOfPicNums::Subtract(1)],
        DecRefPicMarking::SlidingWindow,
    );

    let mut references = Vec::new();
    let lists = dpb
        .ref_lists(
            &type0_sps(),
            &synthetic_pps(),
            &header,
            &current(2, 4),
            &mut references,
        )
        .unwrap();

    assert_eq!(lists.l0, vec![0, 1]);
}

/// A modification addressing a frame that isn't in the DPB is an error.
#[test]
fn ref_list_modification_to_missing_frame_is_an_error() {
    let dpb = dpb_with_two_refs();
    let header = p_slice_header(
        2,
        vec![ModificationOfPicNums::Subtract(7)],
        DecRefPicMarking::SlidingWindow,
    );

    let mut references = Vec::new();
    let result = dpb.ref_lists(
        &type0_sps(),
        &synthetic_pps(),
        &header,
        &current(2, 4),
        &mut references,
    );
    assert!(
        matches!(result, Err(ParseError::MissingReference { .. })),
        "{result:?}"
    );
}

/// Long-term references: assignment via memory management control operations 3 and 6,
/// their position after short-term frames in list initialization, addressing via
/// `LongTermRef` modifications, and unmarking via operations 2 and 4 (spec 8.2.5.4).
#[test]
fn long_term_references() {
    // Room for three reference frames: two long-term and one short-term.
    let mut sps = type0_sps();
    sps.max_num_ref_frames = 3;
    let mut dpb = Dpb::default();
    dpb.configure(&sps, 17).unwrap();
    dpb.mark(
        &current(0, 0),
        Some(&DecRefPicMarking::Idr {
            no_output_of_prior_pics_flag: false,
            long_term_reference_flag: false,
        }),
    )
    .unwrap();
    dpb.mark(&current(1, 2), Some(&DecRefPicMarking::SlidingWindow))
        .unwrap();

    // Operation 3: the IDR frame (picNum 2 - 2 = 0) becomes long-term index 0.
    // Operation 6: the current frame becomes long-term index 1.
    let outcome = dpb
        .mark(
            &current(2, 4),
            Some(&DecRefPicMarking::Adaptive(vec![
                MemoryManagementControlOperation::MaxUsedLongTermFrameRef {
                    max_long_term_frame_idx_plus1: 2,
                },
                MemoryManagementControlOperation::ShortTermUsedForLongTerm {
                    difference_of_pic_nums_minus1: 1,
                    long_term_frame_idx: 0,
                },
                MemoryManagementControlOperation::CurrentUsedForLongTerm {
                    long_term_frame_idx: 1,
                },
            ])),
        )
        .unwrap();
    assert_eq!(outcome.setup_slot, Some(2));
    assert_eq!(outcome.long_term_frame_idx, Some(1));
    assert!(outcome.freed.is_empty());

    // List order: the short-term frame first, then long-term by ascending index.
    let header = p_slice_header(3, vec![], DecRefPicMarking::SlidingWindow);
    let mut references = Vec::new();
    let lists = dpb
        .ref_lists(
            &type0_sps(),
            &synthetic_pps(),
            &header,
            &current(3, 6),
            &mut references,
        )
        .unwrap();
    // Slot 1 is short-term (frame_num 1), slot 0 long-term idx 0, slot 2 long-term idx 1.
    assert_eq!(lists.l0, vec![1, 0, 2]);
    let long_term: Vec<&ReferenceInfo> = references
        .iter()
        .filter(|reference| reference.is_long_term)
        .collect();
    assert_eq!(long_term.len(), 2);

    // `LongTermRef` modification addresses by long-term picture number.
    let header = p_slice_header(
        3,
        vec![ModificationOfPicNums::LongTermRef(1)],
        DecRefPicMarking::SlidingWindow,
    );
    let mut references = Vec::new();
    let lists = dpb
        .ref_lists(
            &type0_sps(),
            &synthetic_pps(),
            &header,
            &current(3, 6),
            &mut references,
        )
        .unwrap();
    assert_eq!(lists.l0, vec![2, 1, 0]);

    // Operation 2 unmarks long-term index 1, operation 4 with plus1 = 0 removes the rest.
    let outcome = dpb
        .mark(
            &current(3, 6),
            Some(&DecRefPicMarking::Adaptive(vec![
                MemoryManagementControlOperation::LongTermUnusedForRef {
                    long_term_pic_num: 1,
                },
                MemoryManagementControlOperation::MaxUsedLongTermFrameRef {
                    max_long_term_frame_idx_plus1: 0,
                },
            ])),
        )
        .unwrap();
    assert_eq!(outcome.freed, vec![2, 0]);
}

/// A `frame_num` jump means reference frames were lost, wraparound is not a jump.
#[test]
fn frame_num_gap_detection() {
    let mut dpb = Dpb::default();
    let sps = type0_sps();
    dpb.configure(&sps, 17).unwrap();
    dpb.mark(
        &current(0, 0),
        Some(&DecRefPicMarking::Idr {
            no_output_of_prior_pics_flag: false,
            long_term_reference_flag: false,
        }),
    )
    .unwrap();

    assert!(dpb.check_frame_num(&sps, 0).is_ok());
    assert!(dpb.check_frame_num(&sps, 1).is_ok());
    let result = dpb.check_frame_num(&sps, 2);
    assert!(
        matches!(result, Err(ParseError::FrameNumGap { .. })),
        "{result:?}"
    );

    // Wraparound: after frame_num 15, 0 is the expected successor.
    for frame_num in 1..16 {
        dpb.mark(
            &current(frame_num, 0),
            Some(&DecRefPicMarking::SlidingWindow),
        )
        .unwrap();
    }
    assert!(dpb.check_frame_num(&sps, 0).is_ok());
}

// --- StdVideo parameter conversion ---

fn asset_parameter_sets(name: &str) -> (SeqParameterSet, PicParameterSet) {
    let data = asset(name);
    let mut ctx = h264_reader::Context::new();
    let mut sps = None;
    let mut pps = None;
    for range in re_video_parsing::nal_ranges(&data).unwrap() {
        use h264_reader::nal::{Nal as _, RefNal, UnitType};
        let nal = RefNal::new(&data[range], &[], true);
        match nal.header().unwrap().nal_unit_type() {
            UnitType::SeqParameterSet => {
                let parsed = super::parse::parse_sps(&nal).unwrap();
                ctx.put_seq_param_set(parsed.clone());
                sps = Some(parsed);
            }
            UnitType::PicParameterSet => {
                pps = Some(super::parse::parse_pps(&ctx, &nal).unwrap());
            }
            _ => {}
        }
    }
    (sps.unwrap(), pps.unwrap())
}

/// The `StdVideo` structs carry the parsed SPS/PPS values through unchanged.
#[test]
fn std_parameter_conversion() {
    use ash::vk::native as std_video;

    let (sps, pps) = asset_parameter_sets("ipb");

    let std_sps = SpsStdParams::build(&sps);
    let std = std_sps.std();
    assert_eq!(
        std.profile_idc,
        std_video::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH
    );
    assert_eq!(
        std.chroma_format_idc,
        std_video::StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_420
    );
    assert_eq!(
        std.pic_order_cnt_type,
        std_video::StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_0
    );
    // 64x64 is 4x4 macroblocks.
    assert_eq!(std.pic_width_in_mbs_minus1, 3);
    assert_eq!(std.pic_height_in_map_units_minus1, 3);
    assert_eq!(std.max_num_ref_frames, sps.max_num_ref_frames as u8);
    assert_eq!(std.log2_max_frame_num_minus4, sps.log2_max_frame_num_minus4);
    assert_eq!(std.flags.frame_mbs_only_flag(), 1);
    assert_eq!(std.flags.vui_parameters_present_flag(), 0);
    assert!(std.pSequenceParameterSetVui.is_null());
    assert!(std.pScalingLists.is_null());
    assert!(std.pOffsetForRefFrame.is_null());

    let std_pps = PpsStdParams::build(&pps);
    let std = std_pps.std();
    assert_eq!(std.seq_parameter_set_id, 0);
    assert_eq!(std.pic_parameter_set_id, 0);
    assert_eq!(
        std.flags.entropy_coding_mode_flag(),
        u32::from(pps.entropy_coding_mode_flag)
    );
    assert_eq!(
        std.flags.weighted_pred_flag(),
        u32::from(pps.weighted_pred_flag)
    );
    assert_eq!(std.weighted_bipred_idc, u32::from(pps.weighted_bipred_idc));
    assert_eq!(
        std.num_ref_idx_l0_default_active_minus1,
        pps.num_ref_idx_l0_default_active_minus1 as u8
    );
    assert_eq!(std.chroma_qp_index_offset, pps.chroma_qp_index_offset as i8);
    assert_eq!(
        std.flags.transform_8x8_mode_flag(),
        u32::from(
            pps.extension
                .as_ref()
                .is_some_and(|extension| extension.transform_8x8_mode_flag)
        )
    );
}

/// Scaling lists translate into the present/use-default masks and value arrays,
/// and `pScalingLists` points at the boxed copy the struct owns.
#[test]
fn std_sps_scaling_lists() {
    use ash::vk::native as std_video;
    use h264_reader::nal::sps::{ScalingList, SeqScalingMatrix};
    use std::num::NonZeroU8;

    let seven = NonZeroU8::new(7).unwrap();
    let nine = NonZeroU8::new(9).unwrap();
    let mut sps = type0_sps();
    sps.chroma_info.scaling_matrix = Some(SeqScalingMatrix {
        scaling_list4x4: vec![
            ScalingList::List([seven; 16]),
            ScalingList::UseDefault,
            ScalingList::NotPresent,
            ScalingList::NotPresent,
            ScalingList::NotPresent,
            ScalingList::NotPresent,
        ],
        scaling_list8x8: vec![ScalingList::List([nine; 64]), ScalingList::NotPresent],
    });

    let std_sps = SpsStdParams::build(&sps);
    assert_eq!(std_sps.std().flags.seq_scaling_matrix_present_flag(), 1);

    let lists = std_sps.scaling_lists().unwrap();
    // 4x4 lists 0 (values) and 1 (default) are present, and 8x8 list 0 at bit 6.
    assert_eq!(lists.scaling_list_present_mask, 0b100_0011);
    assert_eq!(lists.use_default_scaling_matrix_mask, 0b000_0010);
    assert_eq!(lists.ScalingList4x4[0], [7; 16]);
    assert_eq!(lists.ScalingList8x8[0], [9; 64]);

    let expected: *const std_video::StdVideoH264ScalingLists = lists;
    assert_eq!(std_sps.std().pScalingLists, expected);
}

/// Bitstream `level_idc` numbers map onto the `StdVideo` level enum,
/// unknown values rounding up.
#[test]
fn std_level_mapping() {
    use ash::vk::native as std_video;

    let level = |level_idc: u8| {
        let mut sps = type0_sps();
        sps.level_idc = level_idc;
        SpsStdParams::build(&sps).std().level_idc
    };

    assert_eq!(
        level(10),
        std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_0
    );
    assert_eq!(
        level(41),
        std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_1
    );
    assert_eq!(
        level(62),
        std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_2
    );
    // 1b rounds up to 1.1, and anything beyond the table clamps to the highest level.
    assert_eq!(
        level(9),
        std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_1
    );
    assert_eq!(
        level(99),
        std_video::StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_2
    );
}
