use super::ffmpeg::Error;

/// Wraps VP8 / VP9 chunks in an IVF stream.
///
/// Their raw bitstream has no self-delimiting framing and `FFmpeg` has no `vp8`/`vp9` raw demuxer.
/// See <https://wiki.multimedia.cx/index.php/IVF>.
pub struct IvfStream {
    /// `b"VP80"` for VP8, `b"VP90"` for VP9.
    fourcc: [u8; 4],
    width: u16,
    height: u16,

    /// Whether we already wrote the 32-byte file header to `FFmpeg`'s stdin.
    file_header_written: bool,

    /// Frame counter used as the per-frame IVF PTS.
    ///
    /// `FFmpeg` is run with `-fps_mode passthrough`, so absolute PTS values
    /// are immaterial as long as they are monotonically increasing.
    frame_idx: u64,
}

impl IvfStream {
    /// `fourcc` is `b"VP80"` for VP8, `b"VP90"` for VP9.
    pub fn new(fourcc: [u8; 4], width: u16, height: u16) -> Self {
        Self {
            fourcc,
            width,
            height,
            file_header_written: false,
            frame_idx: 0,
        }
    }

    /// Write `frame` as one IVF frame, preceded by the IVF file header if this is the first frame.
    pub fn write_frame(&mut self, out: &mut dyn std::io::Write, frame: &[u8]) -> Result<(), Error> {
        let Self {
            fourcc,
            width,
            height,
            file_header_written,
            frame_idx,
        } = self;

        if !*file_header_written {
            write_ivf_file_header(out, fourcc, width, height)?;
            *file_header_written = true;
        }

        write_ivf_frame_header(out, frame.len() as u32, *frame_idx)?;
        out.write_all(frame).map_err(Error::FailedToWriteToFfmpeg)?;

        *frame_idx += 1;

        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::IvfStream;

    const FILE_HEADER_LEN: usize = 32;
    const FRAME_HEADER_LEN: usize = 12;

    #[test]
    fn file_header_once_then_numbered_frames() {
        let mut ivf = IvfStream::new(*b"VP90", 640, 480);
        let mut out = Vec::new();
        ivf.write_frame(&mut out, &[1, 2, 3]).unwrap();
        ivf.write_frame(&mut out, &[4, 5]).unwrap();

        let (file_header, frames) = out.split_at(FILE_HEADER_LEN);
        assert_eq!(&file_header[0..4], b"DKIF");
        assert_eq!(&file_header[8..12], b"VP90");
        assert_eq!(&file_header[12..14], &640u16.to_le_bytes());
        assert_eq!(&file_header[14..16], &480u16.to_le_bytes());

        let (first, second) = frames.split_at(FRAME_HEADER_LEN + 3);
        assert_eq!(&first[0..4], &3u32.to_le_bytes(), "frame size");
        assert_eq!(&first[4..12], &0u64.to_le_bytes(), "frame index");
        assert_eq!(&first[FRAME_HEADER_LEN..], &[1, 2, 3]);

        assert_eq!(second.len(), FRAME_HEADER_LEN + 2, "no second file header");
        assert_eq!(&second[0..4], &2u32.to_le_bytes(), "frame size");
        assert_eq!(&second[4..12], &1u64.to_le_bytes(), "frame index");
        assert_eq!(&second[FRAME_HEADER_LEN..], &[4, 5]);
    }
}
