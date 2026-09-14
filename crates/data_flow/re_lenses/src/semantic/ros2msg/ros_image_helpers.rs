//! Helpers for converting ROS image messages to Rerun image components.
//!
//! Image helpers that are not specific to ROS should go in [`crate::semantic::image_helpers`]
//! instead.

use arrow::array::{Array as _, ArrayRef, StringArray, StructArray};
use re_lenses_core::combinators::{Error, try_downcast};

use crate::semantic::helpers::get_field_as;

use super::super::image_helpers::{
    encoding_to_image_format, extract_image_buffer, is_single_channel_encoding,
};

const ENCODING_FIELD: &str = "encoding";

/// The Rerun image archetype corresponding to a ROS image encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageKind {
    Color,
    Depth,
}

/// Returns a pipe-compatible function that conditionally converts a ROS image message to a Rerun
/// image format.
///
/// The output is suppressed when all messages have the other image kind.
pub(crate) fn encoding_to_image_format_if(
    expected_kind: ImageKind,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        if classify_image_kind(source)? == Some(expected_kind) {
            encoding_to_image_format()(source)
        } else {
            Ok(None)
        }
    }
}

/// Returns a pipe-compatible function that conditionally extracts a ROS image buffer.
///
/// The output is suppressed when all messages have the other image kind.
pub(crate) fn extract_image_buffer_if(
    expected_kind: ImageKind,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        if classify_image_kind(source)? == Some(expected_kind) {
            extract_image_buffer()(source)
        } else {
            Ok(None)
        }
    }
}

fn classify_image_kind(source: &ArrayRef) -> Result<Option<ImageKind>, Error> {
    let source = try_downcast::<StructArray>(source, "classify_image_kind input")?;
    let encoding_array = get_field_as::<StringArray>(source, ENCODING_FIELD)?;

    let mut kind = None;
    for i in 0..source.len() {
        if encoding_array.is_null(i) {
            return Err(Error::UnexpectedNull {
                field_name: ENCODING_FIELD,
                context: "classify_image_kind",
            });
        }

        let current = if is_single_channel_encoding(encoding_array.value(i))? {
            // Note: we might incorrectly classify grayscale images as depth images here
            ImageKind::Depth
        } else {
            ImageKind::Color
        };

        match kind {
            Some(previous) if previous != current => {
                return Err(Error::Other(
                    "Mixed color and depth image encodings in the same chunk are not supported"
                        .to_owned(),
                ));
            }
            Some(_) => {}
            None => kind = Some(current),
        }
    }

    Ok(kind)
}
