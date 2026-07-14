use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, VideoStream};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CompressedVideo`] messages.
///
/// [`foxglove.CompressedVideo`]: https://docs.foxglove.dev/docs/sdk/schemas/compressed-video
pub fn compressed_video(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("foxglove.CompressedVideo:message")
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
            VideoStream::descriptor_codec(),
            Selector::parse(".format")?.pipe(op::string_to_video_codec()),
        )
        .to_component(
            VideoStream::descriptor_sample(),
            Selector::parse(".data")?.pipe(op::binary_to_list_uint8()),
        )
        .build()
}
