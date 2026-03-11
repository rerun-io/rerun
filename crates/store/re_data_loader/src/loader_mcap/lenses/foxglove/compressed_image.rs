use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, EncodedImage};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CompressedImage`] messages.
///
/// [`foxglove.CompressedImage`]: https://docs.foxglove.dev/docs/sdk/schemas/compressed-image
pub fn compressed_image(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    time_type,
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?
                        .then(MapList::new(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX))),
                )?
                // The format field can be "jpeg", "png", "webp" or "avif" in the Foxglove schema.
                // We prefix with "image/" to get valid MIME types for Rerun.
                .component(
                    EncodedImage::descriptor_media_type(),
                    Selector::parse(".format")?.then(MapList::new(op::string_prefix("image/"))),
                )?
                .component(
                    EncodedImage::descriptor_blob(),
                    Selector::parse(".data")?.then(MapList::new(op::binary_to_list_uint8())),
                )
            })?
            .build(),
    )
}
