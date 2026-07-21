use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::Pinhole;

use crate::importer_mcap::lenses::helpers::row_major_3x3_to_column_major;

use super::IMAGE_PLANE_SUFFIX;

/// Creates a lens for `sensor_msgs/msg/CameraInfo` messages.
pub fn camera_info() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.CameraInfo:message")
        .to_component(
            Pinhole::descriptor_child_frame(),
            Selector::parse(".header.frame_id")?
                .pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
        )
        .to_component_with_cast(
            Pinhole::descriptor_resolution(),
            Selector::parse("pack(.width!, .height!)")?,
            CastTo::Auto,
        )
        .to_component(
            Pinhole::descriptor_image_from_camera(),
            Selector::parse(".k")?.pipe(row_major_3x3_to_column_major()),
        )
        .to_component(
            Pinhole::descriptor_parent_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .build()
}
