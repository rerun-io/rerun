use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, Image};

use crate::loader_mcap::lenses::image_helpers::{encoding_to_image_format, extract_image_buffer};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.RawImage`] messages.
///
/// [`foxglove.RawImage`]: https://docs.foxglove.dev/docs/sdk/schemas/raw-image
pub fn raw_image() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.RawImage:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
                )
                .component(
                    CoordinateFrame::descriptor_frame(),
                    [
                        Op::selector(".frame_id"),
                        Op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX),
                    ],
                )
                .component(
                    Image::descriptor_format(),
                    [Op::func(encoding_to_image_format)],
                )
                .component(Image::descriptor_buffer(), [Op::func(extract_image_buffer)])
            })?
            .build(),
    )
}
