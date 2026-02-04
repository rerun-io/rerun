use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, EncodedImage};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CompressedImage`] messages.
///
/// [`foxglove.CompressedImage`]: https://docs.foxglove.dev/docs/sdk/schemas/compressed-image
pub fn compressed_image() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
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
                        Op::string_suffix(IMAGE_PLANE_SUFFIX),
                    ],
                )
                // The format field can be "jpeg", "png", "webp" or "avif" in the Foxglove schema.
                // We prefix with "image/" to get valid MIME types for Rerun.
                .component(
                    EncodedImage::descriptor_media_type(),
                    [Op::selector(".format"), Op::string_prefix("image/")],
                )
                .component(
                    EncodedImage::descriptor_blob(),
                    [Op::selector(".data"), Op::binary_to_list_uint8()],
                )
            })?
            .build(),
    )
}
