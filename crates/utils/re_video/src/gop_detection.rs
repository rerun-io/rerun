use crate::av1::detect_av1_keyframe_start;
use crate::h264::detect_h264_annexb_gop;
use crate::h265::detect_h265_annexb_gop;
use crate::{VideoCodec, VideoEncodingDetails};

/// Failure reason for [`detect_gop_start`].
#[derive(thiserror::Error, Debug)]
pub enum DetectGopStartError {
    #[error("Detection not supported for codec: {0:?}")]
    UnsupportedCodec(VideoCodec),

    #[error("NAL header error: {0:?}")]
    NalHeaderError(h264_reader::nal::NalHeaderError),

    #[error("AV1 parser error: {0}")]
    Av1ParserError(std::io::Error),

    #[error("Detected group of picture but failed to extract encoding details: {0:?}")]
    FailedToExtractEncodingDetails(String),
}

impl PartialEq<Self> for DetectGopStartError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::UnsupportedCodec(a), Self::UnsupportedCodec(b)) => a == b,
            (Self::NalHeaderError(_), Self::NalHeaderError(_)) => true, // `NalHeaderError` isn't implementing PartialEq, but there's only one variant.
            (Self::FailedToExtractEncodingDetails(a), Self::FailedToExtractEncodingDetails(b)) => {
                a == b
            }
            _ => false,
        }
    }
}

impl Eq for DetectGopStartError {}

/// Result of a successful GOP detection.
///
/// I.e. whether a sample is the start of a GOP and if so, encoding details we were able to extract from it.
#[derive(Default, PartialEq, Eq, Debug)]
pub enum GopStartDetection {
    /// The sample is the start of a GOP and encoding details have been extracted.
    StartOfGop(VideoEncodingDetails),

    /// The sample is not the start of a GOP.
    #[default]
    NotStartOfGop,
}

impl GopStartDetection {
    #[inline]
    pub fn is_start_of_gop(&self) -> bool {
        matches!(self, Self::StartOfGop(_))
    }
}

/// Try to determine whether a frame chunk is the start of a GOP.
///
/// This is a best effort attempt to determine this, but we won't always be able to.
#[inline]
pub fn detect_gop_start(
    sample_data: &[u8],
    codec: VideoCodec,
) -> Result<GopStartDetection, DetectGopStartError> {
    #[expect(clippy::match_same_arms)]
    match codec {
        VideoCodec::H264 => detect_h264_annexb_gop(sample_data),
        VideoCodec::H265 => detect_h265_annexb_gop(sample_data),
        VideoCodec::AV1 => detect_av1_keyframe_start(sample_data),
        VideoCodec::VP8 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::VP9 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::ImageSequence(codec) => {
            // Images are always treated as keyframes.
            // Each meta function checks magic bytes internally and returns
            // `WrongFormat` if they don't match vs `InvalidData` if parsing fails.
            let (codec_string, meta) = match codec {
                Some(codec) if codec == "image/png" => (codec, png_meta(sample_data)?),
                Some(codec) if codec == "image/jpeg" => (codec, jpeg_meta(sample_data)?),
                None => guess_image_meta(sample_data)?,
                Some(_) => {
                    return Err(DetectGopStartError::UnsupportedCodec(
                        VideoCodec::ImageSequence(codec),
                    ));
                }
            };
            Ok(GopStartDetection::StartOfGop(VideoEncodingDetails {
                codec_string,
                coded_dimensions: meta.coded_dimensions,
                bit_depth: meta.bit_depth,
                chroma_subsampling: None,
                stsd: None,
            }))
        }
    }
}

/// Try getting the metadata of the image as all supported formats.
///
/// Returns the format string and metadata.
fn guess_image_meta(sample_data: &[u8]) -> Result<(String, ImageMeta), ImageSizeError> {
    type MetaFn = fn(&[u8]) -> Result<ImageMeta, ImageSizeError>;
    let formats: &[(&str, MetaFn)] = &[("image/png", png_meta), ("image/jpeg", jpeg_meta)];

    for &(name, meta_fn) in formats {
        match meta_fn(sample_data) {
            Ok(meta) => return Ok((name.to_owned(), meta)),
            // Try the next format if magic bytes didn't match.
            Err(ImageSizeError::WrongFormat(_)) => {}
            Err(other) => {
                return Err(other);
            }
        }
    }

    Err(ImageSizeError::WrongFormat(String::new()))
}

struct ImageMeta {
    coded_dimensions: [u16; 2],
    bit_depth: Option<u8>,
}

enum ImageSizeError {
    /// The magic bytes didn't match this format.
    WrongFormat(String),

    /// The magic bytes matched but we couldn't extract the size.
    InvalidData(String),
}

impl From<ImageSizeError> for DetectGopStartError {
    fn from(err: ImageSizeError) -> Self {
        match err {
            ImageSizeError::WrongFormat(for_format) => {
                Self::FailedToExtractEncodingDetails(match for_format.as_str() {
                    "" => {
                        "Image data doesn't match any supported image format (image/png, image/jpeg)".to_owned()
                    }
                    _ => {
                        format!(
                            "Image didn't match the specified image format '{}'",
                            for_format.as_str()
                        )
                    }
                })
            }
            ImageSizeError::InvalidData(msg) => Self::FailedToExtractEncodingDetails(msg),
        }
    }
}

/// Extract width, height, and bit depth from a PNG.
///
/// See <https://www.w3.org/TR/png/#5DataRep> for the PNG file structure.
/// The IHDR chunk layout after the 8-byte signature and 8-byte chunk header is:
///   16..20 width, 20..24 height, 24 bit depth, 25 color type.
fn png_meta(sample_data: &[u8]) -> Result<ImageMeta, ImageSizeError> {
    const PNG_MAGIC_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n";

    if sample_data.get(..8) != Some(PNG_MAGIC_BYTES) {
        return Err(ImageSizeError::WrongFormat("image/png".to_owned()));
    }
    let convert_size = |e: Option<&[u8]>| {
        u32::from_be_bytes(
            e.ok_or_else(|| {
                ImageSizeError::InvalidData("Invalid PNG data, couldn't extract size".to_owned())
            })?
            .try_into()
            .expect("This is 4 bytes"),
        )
        .try_into()
        .map_err(|_err| ImageSizeError::InvalidData("PNG image dimension too large".to_owned()))
    };

    let w = convert_size(sample_data.get(16..20))?;
    let h = convert_size(sample_data.get(20..24))?;
    let bit_depth = sample_data.get(24).copied();

    Ok(ImageMeta {
        coded_dimensions: [w, h],
        bit_depth,
    })
}

/// Extract width, height, and bit depth from a JPEG by scanning for a Start of Frame marker.
///
/// See <https://www.w3.org/Graphics/JPEG/itu-t81.pdf>, Table B.1 for marker definitions.
/// The SOF segment layout is: precision(1), height(2), width(2), …
fn jpeg_meta(data: &[u8]) -> Result<ImageMeta, ImageSizeError> {
    const JPEG_MAGIC_BYTES: &[u8] = &[0xFF, 0xD8];

    if data.get(..2).is_none_or(|b| b != JPEG_MAGIC_BYTES) {
        return Err(ImageSizeError::WrongFormat("image/jpeg".to_owned()));
    }

    let invalid_data =
        || ImageSizeError::InvalidData("Invalid JPEG data, couldn't extract size".to_owned());

    let mut i = 2;
    loop {
        if *data.get(i).ok_or_else(invalid_data)? != 0xFF {
            return Err(invalid_data());
        }
        while *data.get(i).ok_or_else(invalid_data)? == 0xFF {
            i += 1;
        }
        let tag = *data.get(i).ok_or_else(invalid_data)?;
        i += 1;

        let len = u16::from_be_bytes([
            *data.get(i).ok_or_else(invalid_data)?,
            *data.get(i + 1).ok_or_else(invalid_data)?,
        ]) as usize;
        i += 2;

        // Start of Frame markers: SOF0-SOF3, SOF5-SOF7, SOF9-SOF11, SOF13-SOF15
        let is_sof = matches!(tag, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_sof {
            let bit_depth = *data.get(i).ok_or_else(invalid_data)?;
            let s = data.get(i + 1..i + 5).ok_or_else(invalid_data)?;
            let h = u16::from_be_bytes([s[0], s[1]]);
            let w = u16::from_be_bytes([s[2], s[3]]);
            return Ok(ImageMeta {
                coded_dimensions: [w, h],
                bit_depth: Some(bit_depth),
            });
        }

        i += len - 2;
    }
}
