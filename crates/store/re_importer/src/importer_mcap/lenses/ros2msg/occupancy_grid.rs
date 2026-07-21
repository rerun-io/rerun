use re_lenses::{CastTo, Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, GridMap};
use re_sdk_types::components::Colormap;

use super::ros_map_helpers::{
    default_ros_map_colormap, map_buffer_to_image_buffer, map_dimensions_to_l8_image_format,
};

/// Creates a lens for `nav_msgs/msg/OccupancyGrid` messages.
pub fn occupancy_grid() -> Result<Lens, LensBuilderError> {
    Lens::derive("nav_msgs.msg.OccupancyGrid:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            GridMap::descriptor_data(),
            Selector::parse(".")?.pipe(map_buffer_to_image_buffer("info", "width", "height")),
        )
        .to_component(
            GridMap::descriptor_format(),
            Selector::parse(".info")?.pipe(map_dimensions_to_l8_image_format()),
        )
        .to_component(
            GridMap::descriptor_cell_size(),
            Selector::parse(".info.resolution")?,
        )
        .to_component_with_cast(
            GridMap::descriptor_translation(),
            Selector::parse(".info.origin.position | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            GridMap::descriptor_quaternion(),
            Selector::parse(".info.origin.orientation | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .to_component(
            GridMap::descriptor_colormap(),
            Selector::parse(".")?.pipe(default_ros_map_colormap(Colormap::RvizMap)),
        )
        .build()
}
