use crate::{ChromaSubsamplingModes, GopStartDetection, VideoEncodingDetails};

pub fn detect_vp9_gop(data: &[u8]) -> GopStartDetection {
    let Some(header) = Vp9KeyframeHeader::parse(data) else {
        return GopStartDetection::NotStartOfGop;
    };

    // VP9 uncompressed header has no level; `10` (= Level 1) is a safe placeholder
    // that all WebCodecs implementations accept.
    // See <https://www.webmproject.org/vp9/mp4/>.
    let codec_string = format!("vp09.{:02}.10.{:02}", header.profile, header.bit_depth);
    let chroma_subsampling = match (header.subsampling_x, header.subsampling_y) {
        // 4:0:0 (monochrome) is signalled at the container level, not the bitstream;
        // the VP9 uncompressed header always reports a YUV layout.
        (true, true) => Some(ChromaSubsamplingModes::Yuv420),
        (true, false) => Some(ChromaSubsamplingModes::Yuv422),
        (false, true) => None, // Y-only subsampling is unheard of (mirrors mp4.rs::subsampling_mode).
        (false, false) => Some(ChromaSubsamplingModes::Yuv444),
    };

    GopStartDetection::StartOfGop(VideoEncodingDetails {
        codec_string,
        coded_dimensions: [header.width, header.height],
        bit_depth: Some(header.bit_depth),
        chroma_subsampling,
        stsd: None,
    })
}

struct Vp9KeyframeHeader {
    profile: u8,
    bit_depth: u8,
    subsampling_x: bool,
    subsampling_y: bool,
    width: u16,
    height: u16,
}

impl Vp9KeyframeHeader {
    fn parse(data: &[u8]) -> Option<Self> {
        let mut reader = BitReader::new(data);

        let frame_marker = reader.read_bits(2)?;
        if frame_marker != 0b10 {
            return None;
        }

        let profile_low_bit = reader.read_bits(1)?;
        let profile_high_bit = reader.read_bits(1)?;
        let profile = ((profile_high_bit << 1) | profile_low_bit) as u8;
        if profile == 3 {
            // Reserved zero bit.
            reader.read_bits(1)?;
        }

        let show_existing_frame = reader.read_bits(1)? != 0;
        if show_existing_frame {
            return None;
        }

        let frame_type = reader.read_bits(1)?;
        if frame_type != 0 {
            return None;
        }

        // show_frame and error_resilient_mode.
        reader.read_bits(2)?;

        let sync_code = reader.read_bits(24)?;
        if sync_code != 0x498342 {
            return None;
        }

        let bit_depth = if matches!(profile, 2 | 3) {
            if reader.read_bits(1)? != 0 { 12 } else { 10 }
        } else {
            8
        };

        let color_space = reader.read_bits(3)?;
        let (subsampling_x, subsampling_y) = if color_space != 7 {
            // color_range
            reader.read_bits(1)?;

            if matches!(profile, 1 | 3) {
                let subsampling_x = reader.read_bits(1)? != 0;
                let subsampling_y = reader.read_bits(1)? != 0;
                // Reserved zero bit.
                reader.read_bits(1)?;
                (subsampling_x, subsampling_y)
            } else {
                (true, true)
            }
        } else if matches!(profile, 1 | 3) {
            // sRGB is always full range and 4:4:4.
            let subsampling_x = false;
            let subsampling_y = false;
            // Reserved zero bit.
            reader.read_bits(1)?;
            (subsampling_x, subsampling_y)
        } else {
            (false, false)
        };

        let width = u16::try_from(reader.read_bits(16)? + 1).ok()?;
        let height = u16::try_from(reader.read_bits(16)? + 1).ok()?;

        Some(Self {
            profile,
            bit_depth,
            subsampling_x,
            subsampling_y,
            width,
            height,
        })
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn read_bits(&mut self, num_bits: usize) -> Option<u32> {
        let mut out = 0;

        for _ in 0..num_bits {
            let byte = *self.data.get(self.bit_pos / 8)?;
            let bit = (byte >> (7 - self.bit_pos % 8)) & 1;
            out = (out << 1) | u32::from(bit);
            self.bit_pos += 1;
        }

        Some(out)
    }
}

/// Returns `true` if the raw VP9 frame data begins with a keyframe.
///
/// Checks the frame marker (2 bits == 0b10) and the `frame_type` bit (0 == keyframe).
/// See VP9 bitstream spec §6.2.
pub fn vp9_is_keyframe(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let mut reader = BitReader::new(data);
    // frame_marker: 2 bits, must be 0b10
    let Some(frame_marker) = reader.read_bits(2) else {
        return false;
    };
    if frame_marker != 0b10 {
        return false;
    }
    // profile: 2 bits (+ 1 reserved bit for profile 3)
    let profile_low_bit = reader.read_bits(1).unwrap_or(0);
    let profile_high_bit = reader.read_bits(1).unwrap_or(0);
    let profile = (profile_high_bit << 1) | profile_low_bit;
    if profile == 3 {
        reader.read_bits(1); // reserved zero bit
    }
    // show_existing_frame
    let Some(show_existing_frame) = reader.read_bits(1) else {
        return false;
    };
    if show_existing_frame != 0 {
        return false;
    }
    // frame_type: 0 = keyframe, 1 = interframe
    let Some(frame_type) = reader.read_bits(1) else {
        return false;
    };
    frame_type == 0
}
