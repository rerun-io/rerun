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
import rerun.blueprint as rrb

TIMELINE = "example_time"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="An example of how to load and animate a URDF given some changing joint angles.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    duration = 0.0
    rec = rr.script_setup(args, "rerun_example_animated_urdf")
    rec.set_time(TIMELINE, duration=duration)

    # Log the URDF file once
    urdf_path = Path(__file__).parent.parent.parent / "rust" / "animated_urdf" / "data" / "so100.urdf"
    rec.log_file_from_path(urdf_path)

    # Load the URDF tree structure into memory
    urdf_tree = rr.urdf.UrdfTree.from_file_path(urdf_path)

    # Hide the collision geometries by default in the viewer.
    blueprint = rrb.Grid(
        rrb.Spatial3DView(
            name="Animated URDF", overrides={"so_arm100/collision_geometries": rrb.EntityBehavior(visible=False)}
        )
    )
    rec.send_blueprint(blueprint)

    for step in range(10000):
        for joint_index, joint in enumerate(urdf_tree.joints()):
            if joint.joint_type == "revolute":
                # Usually this angle would come from a measurement, here we just fake something
                sin_value = math.sin(step * (0.02 + joint_index / 100.0))

                # Remap from [-1, 1] to the joint's valid range
                dynamic_angle = joint.limit_lower + (sin_value + 1.0) / 3.0 * (joint.limit_upper - joint.limit_lower)

                # Rerun loads the URDF transforms with child/parent frame relations.
                # To move a joint, we just need to log a new transform between those frames.
                # Here, we use the `compute_transform` method that automatically takes care
                # of setting the frame names and calculating the full transform from the joint angle.
                transform = joint.compute_transform(dynamic_angle)
                rec.log("transforms", transform)

                # We can also work with links from the URDF tree.
                # Here, we change the color of the visual mesh entities of the "jaw" link based on the joint angle.
                link = urdf_tree.get_joint_child(joint)
                if link.name == "jaw":
                    for visual_path in urdf_tree.get_visual_geometry_paths(link):
                        normalized_angle = (dynamic_angle - joint.limit_lower) / (joint.limit_upper - joint.limit_lower)
                        rgba = [1.0 - normalized_angle, normalized_angle, 0, 0.5]
                        rec.log(visual_path, rr.Asset3D.from_fields(albedo_factor=rgba))

        duration += 0.03
        rec.set_time(TIMELINE, duration=duration)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
