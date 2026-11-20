"""Inspect a URDF model with `rr.urdf` and log its joint transforms."""

from pathlib import Path

import rerun as rr

urdf_path = Path(__file__).parent / "minimal.urdf"

rr.init("rerun_example_urdf_utilities", spawn=True)

# region: urdf_tree
urdf_tree = rr.urdf.UrdfTree.from_file_path(urdf_path, entity_path_prefix=None)

# Access properties
robot_name = urdf_tree.name
root_link = urdf_tree.root_link()
joints = urdf_tree.joints()

# Lookup by name
joint = urdf_tree.get_joint_by_name("joint1")
link = urdf_tree.get_link_by_name("base_link")

# Get the entity paths of collision or visual geometries of a link.
# This can be used for example to update the color / transparency
# during runtime:
for visual_path in urdf_tree.get_visual_geometry_paths("tool_link"):
    rr.log(
        visual_path,
        rr.Asset3D.from_fields(albedo_factor=[255, 0, 0, 100]),
        static=True,
    )
# endregion: urdf_tree

# region: frame_prefix
left = rr.urdf.UrdfTree.from_file_path(
    urdf_path, entity_path_prefix="left", frame_prefix="left/"
)
right = rr.urdf.UrdfTree.from_file_path(
    urdf_path, entity_path_prefix="right", frame_prefix="right/"
)

left.log_to_recording()
right.log_to_recording()
# endregion: frame_prefix

assert joint is not None
angle = 0.5

# region: compute_transform
# For revolute/continuous joints: pass angle in radians
# For prismatic joints: pass distance in meters
transform = joint.compute_transform(angle)
rr.log("transforms", transform)
# endregion: compute_transform

# region: compute_transform_columns
angles = [0.0, 0.1, 0.2, 0.3]
rr.send_columns(
    "transforms",
    indexes=[rr.TimeColumn("step", sequence=range(len(angles)))],
    columns=joint.compute_transform_columns(angles),
)
# endregion: compute_transform_columns
