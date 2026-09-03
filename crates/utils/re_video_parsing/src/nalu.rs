use std::ops::Range;

/// The data is not an annex-b NAL stream, most likely it is length-prefixed AVCC data.
#[derive(thiserror::Error, Debug)]
#[error("Data is not an annex-b NAL stream")]
pub struct NotAnnexBError;

/// Splits an annex-b stream into the byte ranges of its NALs, without start codes.
///
/// Handles both 3- and 4-byte start codes and strips trailing zero padding from each NAL.
/// Anything but zero padding before the first start code is an error, it usually means
/// the data is length-prefixed (AVCC) instead of annex-b.
pub fn nal_ranges(data: &[u8]) -> Result<Vec<Range<usize>>, NotAnnexBError> {
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
                return Err(NotAnnexBError);
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
        return Err(NotAnnexBError);
    }

    Ok(ranges)
}

#[derive(Default)]
pub struct AnnexBStreamState {
    pub previous_frame_was_idr: bool,
}

/// In Annex-B before every NAL unit is a NAL start code.
///
/// This is used in Annex-B byte stream formats such as h264 files.
/// Packet transform systems (RTP) may omit these.
///
/// Note that there's also a less commonly used short version with only 2 zeros: `0x00, 0x00, 0x01`.
pub const ANNEXB_NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

#[derive(thiserror::Error, Debug)]
pub enum AnnexBStreamWriteError {
    #[error("Bad video data: {0}")]
    BadVideoData(String),

    #[error("Failed to write to stream: {0}")]
    FailedToWriteToStream(#[from] std::io::Error),
}

pub fn write_length_prefixed_nalus_to_annexb_stream(
    nalu_stream: &mut dyn std::io::Write,
    data: &[u8],
    length_prefix_size: usize,
) -> Result<(), AnnexBStreamWriteError> {
    // A single chunk/sample may consist of multiple NAL units, each of which need our special treatment.
    // (most of the time it's 1:1, but there might be extra NAL units for info, especially at the start).
    let mut buffer_offset: usize = 0;
    let sample_end = data.len();
    while buffer_offset < sample_end {
        re_tracing::profile_scope!("write_nalu");

        if sample_end < buffer_offset + length_prefix_size {
            return Err(AnnexBStreamWriteError::BadVideoData(
                "Not enough bytes to fit the length prefix".to_owned(),
            ));
        }

        let nal_unit_size = match length_prefix_size {
            1 => data[buffer_offset] as usize,

            2 => u16::from_be_bytes(
                #[expect(clippy::unwrap_used)] // can't fail
                data[buffer_offset..(buffer_offset + 2)].try_into().unwrap(),
            ) as usize,

            4 => u32::from_be_bytes(
                #[expect(clippy::unwrap_used)] // can't fail
                data[buffer_offset..(buffer_offset + 4)].try_into().unwrap(),
            ) as usize,

            _ => {
                return Err(AnnexBStreamWriteError::BadVideoData(format!(
                    "Bad length prefix size: {length_prefix_size}"
                )));
            }
        };

        let data_start = buffer_offset + length_prefix_size; // Skip the size.
        let data_end = buffer_offset + nal_unit_size + length_prefix_size;

        if data.len() < data_end {
            return Err(AnnexBStreamWriteError::BadVideoData(
                "Video sample data ends with incomplete NAL unit.".to_owned(),
            ));
        }

        // Can be useful for finding issues, but naturally very spammy.
        // let nal_header = NalHeader(chunk.data[data_start]);
        // re_log::trace!(
        //     "nal_header: {:?}, {}",
        //     nal_header.unit_type(),
        //     nal_header.ref_idc()
        // );

        let data = &data[data_start..data_end];

        nalu_stream.write_all(ANNEXB_NAL_START_CODE)?;

        // Note that we don't have to insert "emulation prevention bytes" since mp4 NALU still use them.
        // (unlike the NAL start code, the presentation bytes are part of the NAL spec!)

        re_tracing::profile_scope!("write_bytes", data.len().to_string());
        nalu_stream.write_all(data)?;

        buffer_offset = data_end;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NotAnnexBError, nal_ranges};

    /// NAL splitting covers both start code lengths, strips the zero padding that
    /// follows a NAL, and drops NALs that are nothing but padding.
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

    /// Data without a leading start code is rejected instead of parsed as one big NAL.
    #[test]
    fn nal_ranges_rejects_non_annexb() {
        // Length-prefixed (AVCC) data has no leading start code.
        assert!(matches!(
            nal_ranges(&[0, 0, 0, 2, 0x65, 0xff]),
            Err(NotAnnexBError)
        ));
        assert!(matches!(nal_ranges(&[0x65, 0xff]), Err(NotAnnexBError)));
    }
}
