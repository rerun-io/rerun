//! Golden-trace tests over ffmpeg-generated fixtures.
//!
//! The reference slot corner cases no common encoder produces are covered by the
//! unit tests next to [`super::refs`] instead.
//!
//! The fixtures live in `tests/assets/`, see `generate_av1.sh` there. Each is an
//! IVF file whose frames are the temporal units pushed one at a time. The traces
//! list every [`DecodeOp`] per temporal unit. Changes to them must be re-reviewed
//! against the spec or a reference decoder.

use std::fmt::Write as _;

use super::{DecodeOp, Parser};

/// Device capacity the tests parse against: the eight reference slots of AV1
/// plus the picture being decoded.
const MAX_DPB_SLOTS: u8 = 9;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/assets/{name}.ivf", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&path).expect("fixture missing, run tests/assets/generate_av1.sh");
    assert!(
        data.len() > 100,
        "Fixture is a stub, git-lfs checkout needed.\nFile path: {path}"
    );
    data
}

/// Splits an IVF file into its frames, one temporal unit each.
fn ivf_frames(data: &[u8]) -> Vec<&[u8]> {
    const FILE_HEADER: usize = 32;
    const FRAME_HEADER: usize = 12;

    assert_eq!(&data[..4], b"DKIF", "not an IVF file");
    let mut frames = Vec::new();
    let mut pos = FILE_HEADER;
    while pos + FRAME_HEADER <= data.len() {
        let size = u32::from_le_bytes(data[pos..pos + 4].try_into().expect("four bytes")) as usize;
        pos += FRAME_HEADER;
        frames.push(&data[pos..pos + size]);
        pos += size;
    }
    assert!(!frames.is_empty(), "fixture holds no frames");
    frames
}

fn trace_fixtures(names: &[&str]) -> String {
    let mut parser = Parser::new(MAX_DPB_SLOTS, false);
    let mut trace = String::new();
    let mut index = 0;
    for name in names {
        for temporal_unit in ivf_frames(&fixture(name)) {
            writeln!(trace, "# TU {index}").unwrap();
            for op in parser.push_access_unit(temporal_unit).unwrap() {
                writeln!(trace, "{op}").unwrap();
            }
            index += 1;
        }
    }
    trace
}

fn trace_fixture(name: &str) -> String {
    trace_fixtures(&[name])
}

#[test]
fn golden_i_only() {
    insta::assert_snapshot!("i_only", trace_fixture("i_only"));
}

#[test]
fn golden_ippp() {
    insta::assert_snapshot!("ippp", trace_fixture("ippp"));
}

/// Alternate reference frames are decoded without being shown and output later
/// by a temporal unit that carries nothing but a `show_existing_frame` header.
#[test]
fn golden_alt_ref() {
    insta::assert_snapshot!("alt_ref", trace_fixture("alt_ref"));
}

#[test]
fn golden_multi_tile() {
    insta::assert_snapshot!("multi_tile", trace_fixture("multi_tile"));
}

/// A second sequence at a different resolution after the first one ended.
#[test]
fn golden_seq_change() {
    insta::assert_snapshot!("seq_change", trace_fixtures(&["ippp", "seq_change"]));
}

/// Decoding must start at a key frame: a stream opening on an inter frame has no
/// references to predict from.
#[test]
fn starting_at_an_inter_frame_is_an_error() {
    let data = fixture("ippp");
    let units = ivf_frames(&data);

    let mut parser = Parser::new(MAX_DPB_SLOTS, false);
    // The first temporal unit carries the sequence header and the key frame,
    // the second an inter frame.
    let inter = units.get(1).expect("fixture has several temporal units");
    assert!(parser.push_access_unit(inter).is_err());
}

/// The sequence header reaches the session parameters once, even though every
/// key frame repeats it.
#[test]
fn sequence_header_is_reported_once() {
    let data = fixture("i_only");
    let mut parser = Parser::new(MAX_DPB_SLOTS, false);

    let mut changes = 0;
    for temporal_unit in ivf_frames(&data) {
        for op in parser.push_access_unit(temporal_unit).unwrap() {
            if matches!(op, DecodeOp::ParametersChanged) {
                changes += 1;
            }
        }
    }
    assert_eq!(changes, 1);
}

/// The conversion carries the parsed sequence header through unchanged.
#[test]
fn std_parameter_conversion() {
    use ash::vk::native as std_video;

    let data = fixture("ippp");
    let mut parser = Parser::new(MAX_DPB_SLOTS, false);
    parser
        .push_access_unit(ivf_frames(&data)[0])
        .expect("the first temporal unit decodes");

    let sequence = parser.std_sequence_header().unwrap();
    assert_eq!(
        sequence.seq_profile,
        std_video::StdVideoAV1Profile_STD_VIDEO_AV1_PROFILE_MAIN
    );
    assert_eq!(sequence.max_frame_width_minus_1, 63);
    assert_eq!(sequence.max_frame_height_minus_1, 63);
    assert!(!sequence.pColorConfig.is_null());

    let facts = parser.seq_facts().unwrap();
    assert_eq!(facts.coded_extent, [64, 64]);
    assert_eq!(facts.dpb_slots, 9);
    assert!(!facts.film_grain_present);
}
