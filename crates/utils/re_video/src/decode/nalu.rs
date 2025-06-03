/// In Annex-B before every NAL unit is a NAL start code.
///
/// Can also be 3 bytes of 0x00, see [`NAL_START_CODE_SHORT`].
///
/// This is used in Annex-B byte stream formats such as h264 files.
/// Packet transform systems (RTP) may omit these.
pub const NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

/// See [`NAL_START_CODE`].
pub const NAL_START_CODE_SHORT: &[u8] = &[0x00, 0x00, 0x01];

/// Given a piece of Annex B encoded data, yields all contained NAL units.
///
/// See <https://membrane.stream/learn/h264/3> for an explanation of Annex B.
/// Skips empty separators, i.e. you can assume that each returned slice will be non-empty.
pub fn iter_annex_b_nal_units(sample_data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let Some(start_pos) = annex_b_next_nal_unit_start_pos(sample_data) else {
        return itertools::Either::Left(std::iter::empty());
    };

    let mut remaining_data = &sample_data[start_pos.separator_pos + start_pos.separator_len..];

    itertools::Either::Right(std::iter::from_fn(move || {
        loop {
            if remaining_data.is_empty() {
                return None;
            }

            let end_pos =
                annex_b_next_nal_unit_start_pos(remaining_data).unwrap_or(AnnexBSeparatorPos {
                    separator_pos: remaining_data.len(),
                    separator_len: 0,
                });

            let nal_unit = &remaining_data[..end_pos.separator_pos];
            remaining_data = &remaining_data[end_pos.separator_pos + end_pos.separator_len..];

            if !nal_unit.is_empty() {
                return Some(nal_unit);
            }
        }
    }))
}

struct AnnexBSeparatorPos {
    separator_pos: usize,
    separator_len: usize,
}

fn annex_b_next_nal_unit_start_pos(sample_data: &[u8]) -> Option<AnnexBSeparatorPos> {
    let mut pos = 0;

    while pos + NAL_START_CODE_SHORT.len() <= sample_data.len() {
        if sample_data[pos..].starts_with(NAL_START_CODE_SHORT) {
            return Some(AnnexBSeparatorPos {
                separator_pos: pos,
                separator_len: NAL_START_CODE_SHORT.len(),
            });
        }
        if sample_data[pos..].starts_with(NAL_START_CODE) {
            return Some(AnnexBSeparatorPos {
                separator_pos: pos,
                separator_len: NAL_START_CODE.len(),
            });
        }

        pos += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::iter_annex_b_nal_units;

    #[test]
    fn test_iter_annex_b_nal_units() {
        let sample_data = b"\x00\x00\x00\x01\x01\x02\x00\x00\x01\x03\x00\x00\x00\x01\x04\x06\x06";
        let mut nal_units = iter_annex_b_nal_units(sample_data);
        assert_eq!(nal_units.next(), Some(b"\x01\x02" as &[u8]));
        assert_eq!(nal_units.next(), Some(b"\x03" as &[u8]));
        assert_eq!(nal_units.next(), Some(b"\x04\x06\x06" as &[u8]));
        assert_eq!(nal_units.next(), None);

        let broken_sample_data = b"\x00\x01\x00\x01";
        assert_eq!(iter_annex_b_nal_units(broken_sample_data).next(), None);

        let zero_size = b"\x00\x00\x00\x01";
        assert_eq!(iter_annex_b_nal_units(zero_size).next(), None);

        let zero_size = b"\x00\x00\x01";
        assert_eq!(iter_annex_b_nal_units(zero_size).next(), None);

        let zero_size_2x = b"\x00\x00\x01\x00\x00\x01";
        assert_eq!(iter_annex_b_nal_units(zero_size_2x).next(), None);
    }
}
