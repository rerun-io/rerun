//! TODO(emilk): replace this whole file with <https://docs.rs/h264-reader/latest/h264_reader/nal/sps/struct.SeqParameterSet.html>

use crate::decode::YuvPixelLayout;

use crate::decode::nalu::{NalHeader, NalUnitType};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum SpsParsingError {
    #[error("SPS buffer too small")]
    UnexpectedEndOfSpsBuffer,

    #[error("Invalid pixel layout index in SPS: chroma_format_idc was {0}.")]
    InvalidPixelLayout(u32),

    #[error("More than one SPS in AVC configuration.")]
    MoreThanOneSpsInAvcc,

    #[error("AVC configuration did not contain a SPS.")]
    NoSpsInAvcc,
}

/// Sequence Parameter Set for h264 video
///
/// Semantics are defined in [ITU-T H.264 (04/2017)](http://wikil.lwwhome.cn:28080/wp-content/uploads/2018/08/T-REC-H.264-201704%E8%8B%B1%E6%96%87.pdf)
#[derive(Debug)]
#[allow(dead_code)]
pub struct H264Sps {
    pub profile_idc: u32,
    pub constraint_set0_flags: bool,
    pub constraint_set1_flags: bool,
    pub constraint_set2_flags: bool,
    pub constraint_set3_flags: bool,
    pub constraint_set4_flags: bool,
    pub level_idc: u32,
    pub seq_parameter_set_id: u32,

    pub chroma_format_idc: u32,
    pub separate_color_plane_flag: bool,
    pub bit_depth_luma_minus8: u32,
    pub bit_depth_chroma_minus8: u32,
    pub qpprime_y_zero_transform_bypass_flag: bool,
    pub seq_scaling_list_present_flag: Option<u32>,

    pub log2_max_frame_num_minus4: u32,
    pub pict_order_cnt_type: u32,
    pub max_num_ref_frames: u32,
    pub gaps_in_frame_num_value_allowed_flag: bool,
    pub pic_width_in_mbs_minus1: u32,
    pub pic_height_in_map_units_minus1: u32,
    pub frame_mbs_only_flag: bool,
    pub mb_adaptive_frame_field_flag: bool,
    pub direct_8x8_inference_flag: bool,

    pub frame_crop_left_offset: Option<u32>,
    pub frame_crop_right_offset: Option<u32>,
    pub frame_crop_top_offset: Option<u32>,
    pub frame_crop_bottom_offset: Option<u32>,
}

impl H264Sps {
    /// Parses a sequence parameter set from a buffer.
    pub fn try_parse(buffer: &[u8]) -> Result<Self, SpsParsingError> {
        let mut bit_read_pos = 0;

        // Follows closely the diagram provided by https://stackoverflow.com/a/6477652
        let profile_idc = read_bits(&mut bit_read_pos, buffer, 8)?;
        let constraint_set0_flags = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let constraint_set1_flags = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let constraint_set2_flags = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let constraint_set3_flags = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let constraint_set4_flags = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let _reserved_zero_3bits = read_bits(&mut bit_read_pos, buffer, 3)?;
        let level_idc = read_bits(&mut bit_read_pos, buffer, 8)?;
        let seq_parameter_set_id = read_exponential_golomb(&mut bit_read_pos, buffer)?;

        let (
            chroma_format_idc,
            separate_color_plane_flag,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            qpprime_y_zero_transform_bypass_flag,
            seq_scaling_list_present_flag,
        ) = if profile_idc == 100
            || profile_idc == 110
            || profile_idc == 122
            || profile_idc == 244
            || profile_idc == 44
            || profile_idc == 83
            || profile_idc == 86
            || profile_idc == 118
            || profile_idc == 128
        {
            let chroma_format_idc = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let separate_color_plane_flag = if chroma_format_idc == 3 {
                read_bits(&mut bit_read_pos, buffer, 1)? == 1
            } else {
                false
            };

            let bit_depth_luma_minus8 = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let bit_depth_chroma_minus8 = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let qpprime_y_zero_transform_bypass_flag =
                read_bits(&mut bit_read_pos, buffer, 1)? == 1;
            let seq_scaling_matrix_present_flag = read_bits(&mut bit_read_pos, buffer, 1)? == 1;

            let seq_scaling_list_present_flag = if seq_scaling_matrix_present_flag {
                let num_seq_scaling_list_present_flag = if chroma_format_idc == 3 { 12 } else { 9 };
                Some(read_bits(
                    &mut bit_read_pos,
                    buffer,
                    num_seq_scaling_list_present_flag,
                )?)
            } else {
                None
            };

            (
                chroma_format_idc,
                separate_color_plane_flag,
                bit_depth_luma_minus8,
                bit_depth_chroma_minus8,
                qpprime_y_zero_transform_bypass_flag,
                seq_scaling_list_present_flag,
            )
        } else {
            // TODO(andreas): Not entirely sure about all of these values. Chroma_format_idc is the most important and easiest to find online.
            let chroma_format_idc = 1;
            let separate_color_plane_flag = false;
            let bit_depth_luma_minus8 = 0;
            let bit_depth_chroma_minus8 = 0;
            let qpprime_y_zero_transform_bypass_flag = false;
            let seq_scaling_list_present_flag = None;

            (
                chroma_format_idc,
                separate_color_plane_flag,
                bit_depth_luma_minus8,
                bit_depth_chroma_minus8,
                qpprime_y_zero_transform_bypass_flag,
                seq_scaling_list_present_flag,
            )
        };

        let log2_max_frame_num_minus4 = read_exponential_golomb(&mut bit_read_pos, buffer)?;
        let pict_order_cnt_type = read_exponential_golomb(&mut bit_read_pos, buffer)?;

        // TODO(andreas): skipping over a bunch of stuff here.
        if pict_order_cnt_type == 0 {
            read_exponential_golomb(&mut bit_read_pos, buffer)?;
        } else if pict_order_cnt_type == 1 {
            read_bits(&mut bit_read_pos, buffer, 1)?;
            read_exponential_golomb(&mut bit_read_pos, buffer)?;
            read_exponential_golomb(&mut bit_read_pos, buffer)?;
            for _ in 0..read_exponential_golomb(&mut bit_read_pos, buffer)? {
                read_exponential_golomb(&mut bit_read_pos, buffer)?;
            }
        }

        let max_num_ref_frames = read_exponential_golomb(&mut bit_read_pos, buffer)?;
        let gaps_in_frame_num_value_allowed_flag = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let pic_width_in_mbs_minus1 = read_exponential_golomb(&mut bit_read_pos, buffer)?;
        let pic_height_in_map_units_minus1 = read_exponential_golomb(&mut bit_read_pos, buffer)?;
        let frame_mbs_only_flag = read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let mb_adaptive_frame_field_flag =
            !frame_mbs_only_flag && read_bits(&mut bit_read_pos, buffer, 1)? == 1;
        let direct_8x8_inference_flag = read_bits(&mut bit_read_pos, buffer, 1)? == 1;

        let (
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        ) = if read_bits(&mut bit_read_pos, buffer, 1)? == 1 {
            // frame_cropping_flag
            let frame_crop_left_offset = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let frame_crop_right_offset = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let frame_crop_top_offset = read_exponential_golomb(&mut bit_read_pos, buffer)?;
            let frame_crop_bottom_offset = read_exponential_golomb(&mut bit_read_pos, buffer)?;

            (
                Some(frame_crop_left_offset),
                Some(frame_crop_right_offset),
                Some(frame_crop_top_offset),
                Some(frame_crop_bottom_offset),
            )
        } else {
            (None, None, None, None)
        };

        Ok(Self {
            profile_idc,
            constraint_set0_flags,
            constraint_set1_flags,
            constraint_set2_flags,
            constraint_set3_flags,
            constraint_set4_flags,
            level_idc,
            seq_parameter_set_id,
            chroma_format_idc,
            separate_color_plane_flag,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            qpprime_y_zero_transform_bypass_flag,
            seq_scaling_list_present_flag,

            log2_max_frame_num_minus4,
            pict_order_cnt_type,
            max_num_ref_frames,
            gaps_in_frame_num_value_allowed_flag,
            pic_width_in_mbs_minus1,
            pic_height_in_map_units_minus1,
            frame_mbs_only_flag,
            mb_adaptive_frame_field_flag,
            direct_8x8_inference_flag,

            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        })
    }

    /// Parses a sequence parameter set from an AVC configuration box.
    pub fn parse_from_avcc(avcc: &re_mp4::Avc1Box) -> Result<Self, SpsParsingError> {
        // There might be extensions to the SPS (`NalUnitType::SequenceParameterSetExt`), ignore those.
        let mut sps_units =
            avcc.avcc.sequence_parameter_sets.iter().filter(|sps| {
                NalHeader(sps.bytes[0]).unit_type() == NalUnitType::SequenceParameterSet
            });

        if let Some(sps_unit) = sps_units.next() {
            if sps_units.next().is_some() {
                // This is rather strange. Must mean that some pictures refer to one SPS and some to another!
                // We don't know what to do with this.
                Err(SpsParsingError::MoreThanOneSpsInAvcc)
            } else {
                Self::try_parse(&sps_unit.bytes[1..])
            }
        } else {
            Err(SpsParsingError::NoSpsInAvcc)
        }
    }

    /// Return the pixel layout specified in the SPS.
    ///
    /// None means that the value in the SPS was invalid.
    pub fn pixel_layout(&self) -> Result<YuvPixelLayout, SpsParsingError> {
        // Section 6.1
        // http://wikil.lwwhome.cn:28080/wp-content/uploads/2018/08/T-REC-H.264-201704%E8%8B%B1%E6%96%87.pdf

        match self.chroma_format_idc {
            0 => Ok(YuvPixelLayout::Y400),
            1 => Ok(YuvPixelLayout::Y_U_V420),
            2 => Ok(YuvPixelLayout::Y_U_V422),

            // Spec says:
            // In 4:4:4 sampling, depending on the value of `separate_color_plane_flag``, the following applies:
            // – If `separate_color_plane_flag`` is equal to 0, each of the two chroma arrays has the same height and width as the luma array.
            // – Otherwise (`separate_color_plane_flag`` is equal to 1), the three color planes are separately processed as monochrome sampled pictures
            //
            // So it's planar YUV4:4:4 in either case but in the second the pixel data is spread across frames.
            3 => Ok(YuvPixelLayout::Y_U_V444),

            _ => Err(SpsParsingError::InvalidPixelLayout(self.chroma_format_idc)),
        }
    }
}

fn read_bits(
    bit_read_pos: &mut usize,
    buffer: &[u8],
    num_bits: usize,
) -> Result<u32, SpsParsingError> {
    debug_assert!(num_bits <= 32);

    let highest_byte_read = (num_bits + *bit_read_pos).next_multiple_of(8) / 8;
    if buffer.len() < highest_byte_read {
        return Err(SpsParsingError::UnexpectedEndOfSpsBuffer);
    }

    // Read bit by bit.
    // Obviously this can be sped up by reading bytes when possible, but let's keep it simple.
    let mut result = 0;
    for n in 0..num_bits {
        let bit_pos = *bit_read_pos + n;
        let byte_idx = bit_pos / 8;
        let mask = 1 << (7 - (bit_pos & 7));
        result = result * 2 + (((buffer[byte_idx] & mask) > 0) as u32);
    }

    *bit_read_pos += num_bits;

    Ok(result)
}

/// Reads a sequence of bits in exponential golomb coding
/// See <https://en.wikipedia.org/wiki/Exponential-Golomb_coding>
fn read_exponential_golomb(
    bit_read_pos: &mut usize,
    buffer: &[u8],
) -> Result<u32, SpsParsingError> {
    let mut zero_count = 0;
    while read_bits(bit_read_pos, buffer, 1)? == 0 {
        zero_count += 1;
    }

    Ok(if zero_count == 0 {
        0
    } else {
        // Read that many extra bits.
        let val = read_bits(bit_read_pos, buffer, zero_count)?;

        // Add the 1 bit we already saw in front and subtract 1.
        ((1 << zero_count) | val) - 1
    })
}

#[cfg(test)]
mod tests {
    use super::{SpsParsingError, read_bits, read_exponential_golomb};

    #[test]
    fn test_read_bits() {
        let mut bit_pos = 0;
        assert_eq!(read_bits(&mut bit_pos, &[0b1010_1010], 4).unwrap(), 0b1010);
        assert_eq!(bit_pos, 4);

        let mut bit_pos = 1;
        assert_eq!(read_bits(&mut bit_pos, &[0b0011_0111], 3).unwrap(), 0b011);
        assert_eq!(bit_pos, 4);

        let mut bit_pos = 5;
        assert_eq!(
            read_bits(&mut bit_pos, &[0b0000_0111, 0b1100_0111], 7).unwrap(),
            0b111_1100
        );
        assert_eq!(bit_pos, 12);

        assert_eq!(
            read_bits(&mut 0, &[0], 9),
            Err(SpsParsingError::UnexpectedEndOfSpsBuffer)
        );
        assert_eq!(
            read_bits(&mut 1, &[0], 8),
            Err(SpsParsingError::UnexpectedEndOfSpsBuffer)
        );
    }

    #[test]
    fn test_read_exponential_golomb() {
        let mut bit_pos = 0;
        assert_eq!(
            read_exponential_golomb(&mut bit_pos, &[0b_0001_0001, 0b_0101_0101]).unwrap(),
            7
        );
        assert_eq!(bit_pos, 7);

        let mut bit_pos = 0;
        assert_eq!(
            read_exponential_golomb(&mut bit_pos, &[0b_1010_1010, 0b_1010_1010]).unwrap(),
            0
        );
        assert_eq!(bit_pos, 1);

        let mut bit_pos = 0;
        assert_eq!(
            read_exponential_golomb(&mut bit_pos, &[0b_0011_1111, 0b_1010_1010]).unwrap(),
            6
        );
        assert_eq!(bit_pos, 5);

        let mut bit_pos = 2;
        assert_eq!(
            read_exponential_golomb(&mut bit_pos, &[0b1100_1111]).unwrap(),
            6
        );
        assert_eq!(bit_pos, 7);

        assert_eq!(
            read_exponential_golomb(&mut 0, &[0b_0000_1111]),
            Err(SpsParsingError::UnexpectedEndOfSpsBuffer)
        );
    }
}
