//! Glue around the `cros-codecs` AV1 parser: OBU framing and the checks that
//! keep unsupported streams out.
//!
//! `cros-codecs` walks the temporal unit with its own cursor, so the OBUs come
//! back one at a time and this module tracks their absolute ranges in the pushed
//! buffer, which the bitstream upload and the tile offsets need.

use std::ops::Range;

use cros_codecs::codec::av1::parser::{Obu, ObuAction, Parser, Profile, SequenceHeaderObu};

use crate::ParseError;

/// Where one OBU sits within the pushed temporal unit.
pub struct ObuPosition {
    /// The whole unit: header byte(s), size field, and payload.
    pub range: Range<usize>,

    /// Byte offset of the payload from the start of the unit.
    pub payload_offset: usize,
}

/// Locks the parser to the low-overhead bitstream format.
///
/// The parser otherwise probes the first unit it sees and can mistake a
/// low-overhead temporal unit for a length-prefixed annex-b one. Feeding it a
/// temporal delimiter with an empty payload settles the probe: the second
/// length it would read as an annex-b frame unit size is zero, which annex-b
/// never is.
pub fn lock_to_low_overhead(parser: &mut Parser) {
    const EMPTY_TEMPORAL_DELIMITER: [u8; 2] = [0x12, 0x00];
    let _probe = parser.read_obu(&EMPTY_TEMPORAL_DELIMITER);
}

/// Reads the OBU starting at `pos`, advancing `pos` past it.
///
/// Returns `None` for an OBU the chosen operating point excludes, which is
/// dropped rather than decoded.
pub fn read_obu<'a>(
    parser: &mut Parser,
    data: &'a [u8],
    pos: &mut usize,
) -> Result<Option<(Obu<'a>, ObuPosition)>, ParseError> {
    let rest = data.get(*pos..).unwrap_or_default();
    check_header(rest)?;

    let action = parser
        .read_obu(rest)
        .map_err(|err| ParseError::nal("OBU", err))?;

    let (obu, used) = match action {
        ObuAction::Drop(consumed) => {
            *pos += advance(consumed as usize, rest.len())?;
            return Ok(None);
        }
        ObuAction::Process(obu) => {
            let used = obu.bytes_used;
            (obu, used)
        }
    };

    let used = advance(used, rest.len())?;
    let payload_offset = used - obu.as_ref().len();
    let range = *pos..*pos + used;
    *pos += used;

    Ok(Some((
        obu,
        ObuPosition {
            range,
            payload_offset,
        },
    )))
}

/// Validates the bytes an OBU is consumed at, so the walk always makes progress
/// and stays inside the pushed data.
fn advance(used: usize, available: usize) -> Result<usize, ParseError> {
    if used == 0 || used > available {
        return Err(ParseError::Invalid("OBU size runs past the temporal unit"));
    }
    Ok(used)
}

/// Rejects the OBU headers the parser would panic on, and the framing the
/// backend can't walk.
fn check_header(data: &[u8]) -> Result<(), ParseError> {
    let &header = data
        .first()
        .ok_or(ParseError::Invalid("temporal unit ends mid-OBU"))?;

    if header & 0x80 != 0 {
        return Err(ParseError::Invalid("OBU forbidden bit is set"));
    }
    if header & 0x01 != 0 {
        return Err(ParseError::Invalid("OBU reserved bit is set"));
    }
    if header & 0x02 == 0 {
        // Annex-b framed AV1 carries the sizes outside the units. MP4 samples
        // and `VideoStream` chunks are low-overhead, which always has them.
        return Err(ParseError::Unsupported("OBUs without a size field"));
    }
    Ok(())
}

/// Rejects everything the backend can't decode, checked once per sequence header.
pub fn check_sequence_header(sequence: &SequenceHeaderObu) -> Result<(), ParseError> {
    // 8-bit 4:2:0 only, matching the probed decode profile.
    if sequence.seq_profile != Profile::Profile0 {
        return Err(ParseError::Unsupported(
            "AV1 profiles other than Main (4:2:0 8/10-bit)",
        ));
    }
    let color = &sequence.color_config;
    if color.high_bitdepth {
        return Err(ParseError::Unsupported("bit depth other than 8"));
    }
    if color.mono_chrome {
        return Err(ParseError::Unsupported("monochrome video"));
    }
    if !color.subsampling_x || !color.subsampling_y {
        return Err(ParseError::Unsupported("chroma format other than 4:2:0"));
    }

    // Frame ids let a decoder detect missing references. Vulkan wants the
    // expected id per reference slot, which the parser doesn't derive.
    if sequence.frame_id_numbers_present_flag {
        return Err(ParseError::Unsupported("frame id numbers"));
    }

    Ok(())
}
