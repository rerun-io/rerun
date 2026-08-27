//! Annex-b NAL framing, shared by the codec parsers.

use std::ops::Range;

use crate::ParseError;

/// Splits an annex-b stream into the byte ranges of its NALs, without start codes.
///
/// Handles both 3- and 4-byte start codes and strips trailing zero padding from each NAL.
/// Anything but zero padding before the first start code is an error, it usually means
/// the data is length-prefixed (AVCC/HVCC) instead of annex-b.
pub fn nal_ranges(data: &[u8]) -> Result<Vec<Range<usize>>, ParseError> {
    let mut ranges = Vec::new();

    // Start of the NAL following the most recent start code, if any.
    let mut nal_start = None;

    let mut close_nal = |nal_start: Option<usize>, end: usize| {
        if let Some(start) = nal_start {
            // Zero padding after a NAL is either alignment/`cabac_zero_words`
            // or the leading zero of a 4-byte start code. Neither belongs to the NAL.
            let end = data[start..end]
                .iter()
                .rposition(|&byte| byte != 0)
                .map_or(start, |last_non_zero| start + last_non_zero + 1);
            if end > start {
                ranges.push(start..end);
            }
        }
    };

    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            close_nal(nal_start, i);
            if nal_start.is_none() && data[..i].iter().any(|&byte| byte != 0) {
                return Err(ParseError::NotAnnexB);
            }
            nal_start = Some(i + 3);
            i += 3;
        } else if data[i + 2] > 1 {
            // This byte can be part of no start code, skip past it.
            i += 3;
        } else {
            i += 1;
        }
    }
    close_nal(nal_start, data.len());

    if nal_start.is_none() && data.iter().any(|&byte| byte != 0) {
        return Err(ParseError::NotAnnexB);
    }

    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use super::nal_ranges;
    use crate::ParseError;

    #[test]
    fn nal_ranges_start_codes() {
        // 3- and 4-byte start codes, trailing zero padding, empty input.
        assert_eq!(
            nal_ranges(&[]).unwrap(),
            Vec::<std::ops::Range<usize>>::new()
        );
        assert_eq!(nal_ranges(&[0, 0, 1, 0x65, 0xff]).unwrap(), vec![3..5]);
        assert_eq!(nal_ranges(&[0, 0, 0, 1, 0x65, 0xff]).unwrap(), vec![4..6]);
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0x67, 0x42, 0, 0, 0, 1, 0x68, 0xce]).unwrap(),
            vec![3..5, 9..11]
        );
        // Trailing zeros after the last NAL are stripped.
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0x65, 0xff, 0, 0]).unwrap(),
            vec![3..5]
        );
        // A NAL that is all zeros is dropped entirely.
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0, 0]).unwrap(),
            Vec::<std::ops::Range<usize>>::new()
        );
    }

    #[test]
    fn nal_ranges_rejects_non_annexb() {
        // Length-prefixed (AVCC) data has no leading start code.
        assert!(matches!(
            nal_ranges(&[0, 0, 0, 2, 0x65, 0xff]),
            Err(ParseError::NotAnnexB)
        ));
        assert!(matches!(
            nal_ranges(&[0x65, 0xff]),
            Err(ParseError::NotAnnexB)
        ));
    }
}
