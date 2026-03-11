use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, Image};

use crate::loader_mcap::lenses::image_helpers::{EncodingToImageFormat, ExtractImageBuffer};

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
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?
                        .then(MapList::new(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX))),
                )?
                .component(
                    Image::descriptor_format(),
                    Selector::parse(".")?.then(MapList::new(EncodingToImageFormat)),
                )?
                .component(
                    Image::descriptor_buffer(),
                    Selector::parse(".")?.then(MapList::new(ExtractImageBuffer)),
                )
            })?
            .build(),
    )
}
