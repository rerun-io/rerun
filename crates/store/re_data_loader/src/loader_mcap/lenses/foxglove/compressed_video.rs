use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, VideoStream};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CompressedVideo`] messages.
///
/// [`foxglove.CompressedVideo`]: https://docs.foxglove.dev/docs/sdk/schemas/compressed-video
pub fn compressed_video() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
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
                .component(
                    VideoStream::descriptor_codec(),
                    [Op::selector(".format"), Op::string_to_video_codec()],
                )
                .component(
                    VideoStream::descriptor_sample(),
                    [Op::selector(".data"), Op::binary_to_list_uint8()],
                )
            })?
            .build(),
    )
}
