"""Log a simple occupancy grid map."""

import numpy as np

import rerun as rr

width, height = 64, 64
cell_size = 0.1

# Create a synthetic image with ROS `nav_msgs/OccupancyGrid` cell value conventions:
# -1 (255) unknown, 0 free, 100 occupied.
grid = np.full((height, width), -1, dtype=np.int8)
grid[8:56, 8:56] = 0
grid[20:44, 20:44] = 100

rr.init("rerun_example_grid_map", spawn=True)

rr.log(
    "world/map",
    rr.GridMap(
        data=grid.tobytes(),
        format=rr.components.ImageFormat(
            width=width,
            height=height,
            color_model="L",
            channel_datatype="U8",
        ),
        cell_size=cell_size,
        translation=[-(width * cell_size) / 2.0, -(height * cell_size) / 2.0, 0.0],
        colormap=rr.components.Colormap.RvizMap,
    ),
)
