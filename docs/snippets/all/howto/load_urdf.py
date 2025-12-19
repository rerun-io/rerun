from pathlib import Path

import rerun as rr
from rerun import RecordingStream

with RecordingStream("rerun_example_load_urdf") as rec:
    rec.spawn()

    # `log_file_from_path` automatically uses the built-in URDF data-loader.
    urdf_path = Path(__file__).parent / "minimal.urdf"
    rec.log_file_from_path(urdf_path, static=True)

    # The `flush` call is optional, but it helps with logging consistency,
    # because it ensures that the URDF finishes loading before continuing.
    rec.flush()

    # Later, in your logging code, you'll update the joints using transforms.
    # A minimal example for updating a revolute joint that connects two links:
    joint_axis = [0, 0, 1]  # comes from URDF
    joint_angle = 1.216  # radians
    origin_xyz = [0, 0, 0.1]  # comes from URDF
    # Make sure that `parent_frame` and `child_frame` match the joint's frame IDs in the URDF file.
    rec.log(
        "transforms",
        rr.Transform3D(
            rotation=rr.RotationAxisAngle(axis=joint_axis, angle=joint_angle),
            translation=origin_xyz,
            parent_frame="base_link",
            child_frame="child_link",
        ),
    )
