use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, VideoStream};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CompressedVideo`] messages.
///
/// [`foxglove.CompressedVideo`]: https://docs.foxglove.dev/docs/sdk/schemas/compressed-video
pub fn compressed_video(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
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
                .component(
                    VideoStream::descriptor_codec(),
                    Selector::parse(".format")?.then(MapList::new(op::string_to_video_codec())),
                )?
                .component(
                    VideoStream::descriptor_sample(),
                    Selector::parse(".data")?.then(MapList::new(op::binary_to_list_uint8())),
                )
            })?
            .build(),
    )
}
