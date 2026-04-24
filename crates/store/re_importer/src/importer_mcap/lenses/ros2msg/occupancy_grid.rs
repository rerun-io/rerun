use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, GridMap};
use re_sdk_types::components::Colormap;

use super::ROS2_TIMESTAMP;
use super::ros_map_helpers::{
    default_ros_map_colormap, map_buffer_to_image_buffer, map_dimensions_to_l8_image_format,
};

/// Creates a lens for `nav_msgs/msg/OccupancyGrid` messages.
pub fn occupancy_grid(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column("nav_msgs.msg.OccupancyGrid:message")
        .output_columns(|out| {
            out.time(
                ROS2_TIMESTAMP,
                time_type,
                Selector::parse(".header.stamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(
                CoordinateFrame::descriptor_frame(),
                Selector::parse(".header.frame_id")?,
            )?
            .component(
                GridMap::descriptor_data(),
                Selector::parse(".")?.pipe(map_buffer_to_image_buffer("info", "width", "height")),
            )?
            .component(
                GridMap::descriptor_format(),
                Selector::parse(".info")?.pipe(map_dimensions_to_l8_image_format()),
            )?
            .component(
                GridMap::descriptor_cell_size(),
                Selector::parse(".info.resolution")?,
            )?
            .component(
                GridMap::descriptor_translation(),
                Selector::parse(".info.origin.position")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
            )?
            .component(
                GridMap::descriptor_quaternion(),
                Selector::parse(".info.origin.orientation")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
            )?
            .component(
                GridMap::descriptor_colormap(),
                Selector::parse(".")?.pipe(default_ros_map_colormap(Colormap::RvizMap)),
            )
        })?
        .build())
}
