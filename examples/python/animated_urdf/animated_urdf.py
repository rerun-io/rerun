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
    parser.add_argument(
        "--dual",
        action="store_true",
        help="Load the same URDF twice with different frame prefixes (dual-arm demo).",
    )
    args = parser.parse_args()

    urdf_path = Path(__file__).parent.parent.parent / "rust" / "animated_urdf" / "data" / "so100.urdf"

    if args.dual:
        run_dual(args, urdf_path)
    else:
        run_single(args, urdf_path)


def run_single(args: argparse.Namespace, urdf_path: Path) -> None:
    duration = 0.0
    rec = rr.script_setup(args, "rerun_example_animated_urdf")
    rec.set_time(TIMELINE, duration=duration)

    # Log the URDF file once
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
                dynamic_angle = _fake_angle(joint, step, joint_index, phase=0.0)

                # Rerun loads the URDF transforms with child/parent frame relations.
                # To move a joint, we just need to log a new transform between those frames.
                # Here, we use the `compute_transform` method that automatically takes care
                # of setting the frame names and calculating the full transform from the joint angle.
                transform = joint.compute_transform(dynamic_angle, clamp=True)
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


def run_dual(args: argparse.Namespace, urdf_path: Path) -> None:
    """Load the same URDF twice with different frame prefixes (dual-arm demo)."""
    rec = rr.script_setup(args, "rerun_example_animated_urdf")

    # Load the same URDF twice with different prefixes.
    # - entity_path_prefix separates geometry in the entity tree
    # - frame_prefix makes frame IDs unique ("left/base", "right/base", …)
    left = rr.urdf.UrdfTree.from_file_path(urdf_path, entity_path_prefix="left", frame_prefix="left/")
    right = rr.urdf.UrdfTree.from_file_path(urdf_path, entity_path_prefix="right", frame_prefix="right/")

    # Log both robots (geometry + static transforms with prefixed frame IDs).
    left.log_urdf_to_recording()
    right.log_urdf_to_recording()

    # Offset the arms so they don't overlap.
    rec.log("left", rr.Transform3D(translation=[-0.2, 0, 0]), static=True)
    rec.log("right", rr.Transform3D(translation=[0.2, 0, 0]), static=True)

    blueprint = rrb.Grid(
        rrb.Spatial3DView(
            name="Dual Arm",
            overrides={
                "left/so_arm100/collision_geometries": rrb.EntityBehavior(visible=False),
                "right/so_arm100/collision_geometries": rrb.EntityBehavior(visible=False),
            },
        )
    )
    rec.send_blueprint(blueprint)

    for step in range(10000):
        rec.set_time("step", sequence=step)

        for joint_index, joint in enumerate(left.joints()):
            if joint.joint_type == "revolute":
                angle = _fake_angle(joint, step, joint_index, phase=0.0)
                rec.log("left/joint_transforms", joint.compute_transform(angle, clamp=True))

        for joint_index, joint in enumerate(right.joints()):
            if joint.joint_type == "revolute":
                angle = _fake_angle(joint, step, joint_index, phase=2.0)
                rec.log("right/joint_transforms", joint.compute_transform(angle, clamp=True))

    rr.script_teardown(args)


def _fake_angle(joint: rr.urdf.UrdfJoint, step: int, joint_index: int, phase: float) -> float:
    """Generate a smooth oscillating angle within the joint's limits."""
    sin_value = math.sin(step * (0.02 + joint_index / 100.0) + phase)
    return joint.limit_lower + (sin_value + 1.0) / 3.0 * (joint.limit_upper - joint.limit_lower)


if __name__ == "__main__":
    main()
