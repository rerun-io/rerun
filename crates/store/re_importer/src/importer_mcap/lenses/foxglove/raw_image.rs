use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, Image};

use crate::importer_mcap::lenses::image_helpers::{encoding_to_image_format, extract_image_buffer};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.RawImage`] messages.
///
/// [`foxglove.RawImage`]: https://docs.foxglove.dev/docs/sdk/schemas/raw-image
pub fn raw_image(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column("foxglove.RawImage:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                time_type,
                Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(
                CoordinateFrame::descriptor_frame(),
                Selector::parse(".frame_id")?.pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
            )?
            .component(
                Image::descriptor_format(),
                Selector::parse(".")?.pipe(encoding_to_image_format()),
            )?
            .component(
                Image::descriptor_buffer(),
                Selector::parse(".")?.pipe(extract_image_buffer()),
            )
        })?
        .build())
}
