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
