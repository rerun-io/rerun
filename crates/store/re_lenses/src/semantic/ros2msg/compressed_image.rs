use arrow::array::{Array as _, ArrayRef, StringArray, StructArray};
use itertools::Itertools as _;
use re_lenses_core::Selector;
use re_lenses_core::combinators::{Error, try_downcast};
use re_sdk_types::ToArrow as _;
use re_sdk_types::archetypes::{CoordinateFrame, EncodedDepthImage, EncodedImage, VideoStream};
use re_sdk_types::components::{MediaType, VideoCodec};

use crate::{Lens, LensBuilderError, op};

use super::super::helpers::get_field_as;
use super::super::image_helpers::extract_blob_data;
use super::IMAGE_PLANE_SUFFIX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadKind {
    EncodedImage,
    DepthRvl,
    H264,
}

/// Creates a lens for `sensor_msgs/msg/CompressedImage` messages.
///
/// Emits encoded image, encoded depth image, or video stream components based on the format.
pub fn compressed_image() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.CompressedImage:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?
                .pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
        )
        // Only set for regular encoded images.
        .to_component(
            EncodedImage::descriptor_blob(),
            Selector::parse(".")?.pipe(extract_data_if(PayloadKind::EncodedImage)),
        )
        // Only set for RVL-compressed depth images.
        .to_component(
            EncodedDepthImage::descriptor_blob(),
            Selector::parse(".")?.pipe(extract_data_if(PayloadKind::DepthRvl)),
        )
        // Only set for RVL-compressed depth images.
        .to_component(
            EncodedDepthImage::descriptor_media_type(),
            Selector::parse(".")?.pipe(rvl_media_type_if_depth()),
        )
        // Only set for H.264 video samples.
        .to_component(
            VideoStream::descriptor_sample(),
            Selector::parse(".")?.pipe(extract_data_if(PayloadKind::H264)),
        )
        // Only set for H.264 video samples.
        .to_component(
            VideoStream::descriptor_codec(),
            Selector::parse(".")?.pipe(h264_codec_if_video()),
        )
        .build()
}

/// Returns the compressed payload when every message has the expected payload kind.
///
/// Returns `None` for chunks of a different kind.
fn extract_data_if(
    expected_kind: PayloadKind,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        let source = try_downcast::<StructArray>(source, "extract_compressed_image_data")?;
        if classify_payloads(source)? == Some(expected_kind) {
            extract_blob_data(source)
        } else {
            Ok(None)
        }
    }
}

/// Returns the RVL media type for chunks of compressed depth images.
///
/// Returns `None` for chunks of a different kind.
fn rvl_media_type_if_depth() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync
{
    move |source| {
        let source = try_downcast::<StructArray>(source, "rvl_media_type_if_depth")?;
        if classify_payloads(source)? == Some(PayloadKind::DepthRvl) {
            MediaType::to_arrow(std::iter::repeat_n(MediaType::rvl(), source.len()))
                .map(Some)
                .map_err(|err| Error::Other(err.to_string()))
        } else {
            Ok(None)
        }
    }
}

/// Returns the H.264 video codec for chunks of H.264 video samples.
///
/// Returns `None` for chunks of a different kind.
fn h264_codec_if_video() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        let source = try_downcast::<StructArray>(source, "h264_codec_if_video")?;
        if classify_payloads(source)? == Some(PayloadKind::H264) {
            VideoCodec::to_arrow(std::iter::repeat_n(VideoCodec::H264, source.len()))
                .map(Some)
                .map_err(|err| Error::Other(err.to_string()))
        } else {
            Ok(None)
        }
    }
}

/// Classifies a chunk's payload kind from its ROS compressed-image format strings.
///
/// A chunk must contain one kind because a lens output component cannot represent mixed
/// payload kinds.
fn classify_payloads(source: &StructArray) -> Result<Option<PayloadKind>, Error> {
    let formats = get_field_as::<StringArray>(source, "format")?;
    let kinds = formats
        .iter()
        .map(|format| {
            let format = format.ok_or(Error::UnexpectedNull {
                field_name: "format",
                context: "classify_compressed_image_payload",
            })?;
            Ok(if is_rvl(format) {
                PayloadKind::DepthRvl
            } else if format.eq_ignore_ascii_case("h264") {
                PayloadKind::H264
            } else {
                PayloadKind::EncodedImage
            })
        })
        .try_collect::<_, Vec<_>, Error>()?;

    let kind = kinds.first().copied();
    if kinds.iter().all(|current| Some(*current) == kind) {
        Ok(kind)
    } else {
        Err(Error::Other(
            "Mixed compressed image payload kinds in the same chunk are not supported".to_owned(),
        ))
    }
}

/// Returns whether a ROS compressed-image format denotes RVL-compressed depth data.
fn is_rvl(format: &str) -> bool {
    // Compressed-depth messages put the original pixel encoding before the semicolon and the
    // compression marker and codec after it, e.g. `16UC1; compressedDepth RVL`.
    let Some((encoding, remainder)) = format.split_once(';') else {
        return false;
    };
    if encoding.trim().is_empty() {
        return false;
    }

    let remainder = remainder.trim().to_ascii_lowercase();
    remainder.contains("compresseddepth") && remainder.contains("rvl")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::StringArray;
    use arrow::datatypes::{DataType, Field, Fields};

    use super::*;

    /// Test helper for creating compressed-image messages with the given format strings.
    fn formats(values: Vec<&str>) -> StructArray {
        StructArray::new(
            Fields::from(vec![Field::new("format", DataType::Utf8, false)]),
            vec![Arc::new(StringArray::from(values))],
            None,
        )
    }

    /// Checks that only compressed-depth RVL format strings are classified as RVL.
    #[test]
    fn detects_depth_rvl_format() {
        assert!(is_rvl("16UC1; compressedDepth RVL"));
        assert!(is_rvl("32FC1; compressedDepth RVL"));
        assert!(!is_rvl("16UC1; compressedDepth png"));
        assert!(!is_rvl("jpeg"));
    }

    /// Checks that a chunk cannot mix compressed image payload kinds.
    #[test]
    fn rejects_mixed_payload_kinds() {
        assert!(classify_payloads(&formats(vec!["jpeg", "h264"])).is_err());
    }
}
