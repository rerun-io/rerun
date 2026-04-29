use crate::Chunk;

use super::ffmpeg::Error;

pub fn write_chunk_to_ivf_stream(
    fourcc: &[u8; 4],
    width: &u16,
    height: &u16,
    file_header_written: &mut bool,
    frame_idx: &mut u64,
    out: &mut dyn std::io::Write,
    chunk: &Chunk,
) -> Result<(), Error> {
    if !*file_header_written {
        write_ivf_file_header(out, fourcc, width, height)?;
        *file_header_written = true;
    }

    write_ivf_frame_header(out, chunk.data.len() as u32, *frame_idx)?;
    out.write_all(&chunk.data)
        .map_err(Error::FailedToWriteToFfmpeg)?;

    *frame_idx += 1;

    Ok(())
}

/// Write a 32-byte IVF file header. See <https://wiki.multimedia.cx/index.php/IVF>.
fn write_ivf_file_header(
    out: &mut dyn std::io::Write,
    fourcc: &[u8; 4],
    width: &u16,
    height: &u16,
) -> Result<(), Error> {
    let mut hdr = [0u8; 32];
    hdr[0..4].copy_from_slice(b"DKIF");
    // version=0 left implicit (zeros).
    hdr[6..8].copy_from_slice(&32u16.to_le_bytes()); // header length
    hdr[8..12].copy_from_slice(fourcc);
    hdr[12..14].copy_from_slice(&width.to_le_bytes());
    hdr[14..16].copy_from_slice(&height.to_le_bytes());
    // Placeholder timebase: 1/1000. We run FFmpeg with `-fps_mode passthrough`,
    // so absolute PTS values are immaterial as long as they are monotonic.
    hdr[16..20].copy_from_slice(&60u32.to_le_bytes()); // timebase denominator
    hdr[20..24].copy_from_slice(&1u32.to_le_bytes()); // timebase numerator
    // Advertise an open-ended stream. Some IVF demuxers treat zero as an empty file.
    hdr[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    // unused=0 left implicit (zeros).
    out.write_all(&hdr).map_err(Error::FailedToWriteToFfmpeg)
}

/// Write a 12-byte IVF per-frame header. See <https://wiki.multimedia.cx/index.php/IVF>.
fn write_ivf_frame_header(
    out: &mut dyn std::io::Write,
    frame_size: u32,
    pts: u64,
) -> Result<(), Error> {
    let mut hdr = [0u8; 12];
    hdr[0..4].copy_from_slice(&frame_size.to_le_bytes());
    hdr[4..12].copy_from_slice(&pts.to_le_bytes());
    out.write_all(&hdr).map_err(Error::FailedToWriteToFfmpeg)
}
