#!/usr/bin/env python3
"""
An example of how to load and animate a URDF given some changing joint angles.

Usage:
python -m animated_urdf
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(
        description="An example of how to load and animate a URDF given some changing joint angles.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rec = rr.script_setup(args, "rerun_example_animated_urdf")
    urdf_path = Path(__file__).parent.parent.parent / "rust" / "animated_urdf" / "data" / "so100.urdf"

    # Log the URDF file once, as a static resource
    rec.log_file_from_path(urdf_path, static=True)

    # Load the URDF tree structure into memory
    urdf_tree = rr.urdf.UrdfTree.from_file_path(urdf_path)

    for step in range(10000):
        rec.set_time("step", sequence=step)

        for joint_index, joint in enumerate(urdf_tree.joints()):
            if joint.joint_type == "revolute":
                # Usually this angle would come from a measurement, here we just fake something
                sin_value = math.sin(step * (0.02 + joint_index / 100.0))

                # Remap from [-1, 1] to the joint's valid range
                dynamic_angle = joint.limit_lower + (sin_value + 1.0) / 2.0 * (joint.limit_upper - joint.limit_lower)

                # Rerun loads the URDF transforms with child/parent frame relations.
                # To move a joint, we just need to log a new transform between those frames.
                # Here, we use the `compute_transform` method that automatically takes care
                # of setting the frame names and calculating the full transform from the joint angle.
                transform = joint.compute_transform(dynamic_angle)
                rec.log("transforms", transform)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
