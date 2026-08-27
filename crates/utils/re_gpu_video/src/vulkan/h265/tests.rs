//! Golden-trace tests over ffmpeg-generated fixtures.
//!
//! The reference-set corner cases no common encoder produces (long-term
//! references, counter wraparounds, sets predicting from other sets) are covered
//! by the unit tests next to [`super::poc`] and [`super::rps`] instead.
//!
//! The fixtures live in `tests/assets/`, see `generate.sh` there. The traces list
//! every [`DecodeOp`] per access unit. Changes to them must be re-reviewed against
//! the spec or a reference decoder.

use std::fmt::Write as _;

use super::{DecodeOp, Parser};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/assets/{name}.h265", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&path).expect("fixture missing, run tests/assets/generate.sh");
    assert!(
        data.len() > 100,
        "Fixture is a stub, git-lfs checkout needed.\nFile path: {path}"
    );
    data
}

/// Splits an elementary stream into access units on the access unit delimiters
/// the fixtures are generated with.
fn split_on_aud(data: &[u8]) -> Vec<&[u8]> {
    // An access unit delimiter NAL: a 3-byte start code, then nal type 35 in the
    // upper bits of the two-byte header.
    let cuts: Vec<usize> = data
        .windows(4)
        .enumerate()
        .filter_map(|(index, window)| (window == [0, 0, 1, 35 << 1]).then_some(index))
        .collect();
    assert!(!cuts.is_empty(), "fixture has no access unit delimiters");

    let mut units = Vec::new();
    for (index, &cut) in cuts.iter().enumerate() {
        let end = cuts.get(index + 1).copied().unwrap_or(data.len());
        units.push(&data[cut..end]);
    }
    units
}

fn trace_fixture(name: &str) -> String {
    let mut parser = Parser::new(17);
    let mut trace = String::new();
    for (index, access_unit) in split_on_aud(&fixture(name)).iter().enumerate() {
        writeln!(trace, "# AU {index}").unwrap();
        for op in parser.push_access_unit(access_unit).unwrap() {
            writeln!(trace, "{op}").unwrap();
        }
    }
    writeln!(trace, "# reorder delay {}", parser.reorder_delay()).unwrap();
    trace
}

#[test]
fn golden_i_only() {
    insta::assert_snapshot!("i_only", trace_fixture("i_only"));
}

#[test]
fn golden_ippp() {
    insta::assert_snapshot!("ippp", trace_fixture("ippp"));
}

#[test]
fn golden_ipb() {
    insta::assert_snapshot!("ipb", trace_fixture("ipb"));
}

#[test]
fn golden_ipb_pyramid() {
    insta::assert_snapshot!("ipb_pyramid", trace_fixture("ipb_pyramid"));
}

#[test]
fn golden_multi_slice() {
    insta::assert_snapshot!("multi_slice", trace_fixture("multi_slice"));
}

#[test]
fn golden_sps_change() {
    insta::assert_snapshot!("sps_change", trace_fixture("sps_change"));
}

/// Picture boundaries are detected from the slice segments themselves, so pushing
/// a whole stream at once must decode identically to pushing it one access unit
/// at a time.
#[test]
fn single_push_matches_per_access_unit_pushes() {
    let data = fixture("ipb_pyramid");

    let mut parser = Parser::new(17);
    let mut whole = String::new();
    for op in parser.push_access_unit(&data).unwrap() {
        writeln!(whole, "{op}").unwrap();
    }

    let mut parser = Parser::new(17);
    let mut split = String::new();
    for access_unit in split_on_aud(&data) {
        for op in parser.push_access_unit(access_unit).unwrap() {
            writeln!(split, "{op}").unwrap();
        }
    }

    // The whole-stream push reports the parameter sets once, the split one per
    // access unit that repeats them: compare only the decode operations.
    let decodes = |trace: &str| {
        trace
            .lines()
            .filter(|line| line.starts_with("DecodeFrame"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(decodes(&whole), decodes(&split));
}

/// Decoding must start at a random access point: a stream opening on a trailing
/// picture has no references to predict from.
#[test]
fn starting_at_a_trailing_picture_is_an_error() {
    let data = fixture("ippp");
    let units = split_on_aud(&data);

    let mut parser = Parser::new(17);
    // The first access unit carries the parameter sets and the random access
    // point, the second a trailing picture.
    let trailing = units.get(1).expect("fixture has several access units");
    assert!(parser.push_access_unit(trailing).is_err());
}

/// Every parameter set the stream declares reaches the session parameters, and
/// the repeats encoders put in front of every random access point don't pile up.
#[test]
fn parameter_sets_are_reported_once() {
    let data = fixture("i_only");
    let mut parser = Parser::new(17);

    let mut parameter_changes = 0;
    for access_unit in split_on_aud(&data) {
        for op in parser.push_access_unit(access_unit).unwrap() {
            if matches!(op, DecodeOp::ParametersChanged) {
                parameter_changes += 1;
            }
        }
    }

    // Only the first access unit brings new sets, even though every frame is a
    // random access point that repeats them.
    assert_eq!(parameter_changes, 1);

    let (vps, sps, pps) = parser.std_parameter_sets();
    assert_eq!(vps.len(), 1);
    assert_eq!(sps.len(), 1);
    assert_eq!(pps.len(), 1);
}

/// The conversion carries the parsed sequence parameters through unchanged.
#[test]
fn std_parameter_conversion() {
    use ash::vk::native as std_video;

    let data = fixture("ipb");
    let mut parser = Parser::new(17);
    parser
        .push_access_unit(split_on_aud(&data)[0])
        .expect("the first access unit decodes");

    let (_vps, sps, pps) = parser.std_parameter_sets();
    let sps = &sps[0];
    assert_eq!(
        sps.chroma_format_idc,
        std_video::StdVideoH265ChromaFormatIdc_STD_VIDEO_H265_CHROMA_FORMAT_IDC_420
    );
    assert_eq!(sps.pic_width_in_luma_samples, 64);
    assert_eq!(sps.pic_height_in_luma_samples, 64);
    assert_eq!(sps.bit_depth_luma_minus8, 0);
    assert_eq!(sps.bit_depth_chroma_minus8, 0);
    assert_eq!(sps.flags.vui_parameters_present_flag(), 0);
    assert!(!sps.pProfileTierLevel.is_null());
    assert!(!sps.pDecPicBufMgr.is_null());

    let pps = &pps[0];
    assert_eq!(pps.pps_seq_parameter_set_id, sps.sps_seq_parameter_set_id);

    // The display size follows the conformance window, absent for a size that is
    // already a multiple of the minimum coding block size.
    let facts = parser.sps_facts(sps.sps_seq_parameter_set_id).unwrap();
    assert_eq!(facts.coded_extent, [64, 64]);
    assert_eq!(facts.display, [64, 64]);
    assert_eq!(facts.crop_offset, [0, 0]);
}
