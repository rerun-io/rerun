use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, Image};

use crate::importer_mcap::lenses::image_helpers::{encoding_to_image_format, extract_image_buffer};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.RawImage`] messages.
///
/// [`foxglove.RawImage`]: https://docs.foxglove.dev/docs/sdk/schemas/raw-image
pub fn raw_image(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("foxglove.RawImage:message")
        .to_timeline(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
        )
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id")?.pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
        )
        .to_component(
            Image::descriptor_format(),
            Selector::parse(".")?.pipe(encoding_to_image_format()),
        )
        .to_component(
            Image::descriptor_buffer(),
            Selector::parse(".")?.pipe(extract_image_buffer()),
        )
        .build()
}
