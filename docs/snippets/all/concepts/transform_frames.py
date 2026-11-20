"""Log two-parent frames, implicit `tf#` frames, and a pinhole frustum."""

import rerun as rr

rr.init("rerun_example_transform_frames", spawn=True)

rr.set_time("step", sequence=0)

# region: two_parents
rr.log("tf", rr.Transform3D(parent_frame="first_parent", child_frame="child"))
# Same values as the `second_parent` to `child` transform,
# expressed as `child` to `second_parent`:
rr.log(
    "tf",
    rr.Transform3D(
        parent_frame="child",
        child_frame="second_parent",
        relation=rr.TransformRelation.ChildFromParent,
    ),
)
# endregion: two_parents

rr.set_time("step", sequence=1)

# region: implicit
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("robot/arm", rr.Transform3D(translation=[0, 1, 0]))
rr.log("robot/arm/gripper", rr.Points3D([0, 0, 0]))
# endregion: implicit

rr.set_time("step", sequence=2)

# region: implicit_as_if
rr.log(
    "robot",
    rr.CoordinateFrame("tf#/robot"),
    rr.Transform3D(
        translation=[1, 0, 0], child_frame="tf#/robot", parent_frame="tf#/"
    ),
)
rr.log(
    "robot/arm",
    rr.CoordinateFrame("tf#/robot/arm"),
    rr.Transform3D(
        translation=[0, 1, 0],
        child_frame="tf#/robot/arm",
        parent_frame="tf#/robot",
    ),
)
rr.log(
    "robot/arm/gripper",
    rr.CoordinateFrame("tf#/robot/arm/gripper"),
    rr.Points3D([0, 0, 0]),
)
# endregion: implicit_as_if

rr.set_time("step", sequence=3)

# region: mixed
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log(
    "arm",
    rr.Transform3D(
        translation=[0, 1, 0], parent_frame="tf#/robot", child_frame="arm_frame"
    ),
    rr.CoordinateFrame("arm_frame"),
)
rr.log("gripper", rr.Points3D([0, 0, 0]), rr.CoordinateFrame("arm_frame"))
# endregion: mixed

# region: pinhole_frustum
rr.log(
    "world/camera",
    rr.Pinhole(
        focal_length=500,
        width=640,
        height=480,
        image_plane_distance=0.5,
        color=[255, 128, 0],
        line_width=0.01,
    ),
)
# endregion: pinhole_frustum
