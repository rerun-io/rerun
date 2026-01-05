from __future__ import annotations

import math
from pathlib import Path

import pytest
import rerun as rr
import rerun.urdf as rru

REPO_ROOT = Path(__file__).resolve().parents[3]
URDF_PATH = REPO_ROOT / "examples" / "rust" / "animated_urdf" / "data" / "so100.urdf"


def test_urdf_tree_loading() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    assert tree.name == "so_arm100"

    joints = tree.joints()
    assert len(joints) == 6

    joint = tree.get_joint_by_name("1")
    assert joint is not None
    assert joint.joint_type == "revolute"
    assert joint.parent_link == "base"
    assert joint.child_link == "shoulder"
    assert joint.axis == pytest.approx((0.0, 1.0, 0.0))
    assert joint.limit_lower == pytest.approx(-2.0)
    assert joint.limit_upper == pytest.approx(2.0)

    child_link = tree.get_joint_child(joint)
    assert child_link.name == "shoulder"

    link_path = tree.get_link_path(child_link)
    assert link_path == "/so_arm100/base/1/shoulder"
    assert tree.get_link_path_by_name("shoulder") == link_path

    root_link = tree.root_link()
    assert root_link.name == "base"


def test_urdf_tree_transform() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    roll, pitch, yaw = joint.origin_rpy
    origin_quat = rru._euler_to_quat(roll, pitch, yaw)

    axis_x, axis_y, axis_z = joint.axis

    # Test default (zero) joint value
    transform = joint.compute_transform(0.0)
    expected_quat = rru._quat_multiply(origin_quat, [0.0, 0.0, 0.0, 1.0])
    assert transform.translation == rr.components.Translation3DBatch(rr.components.Translation3D(joint.origin_xyz))
    assert transform.quaternion == rr.components.RotationQuatBatch(rr.components.RotationQuat(xyzw=expected_quat))
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.parent_link)
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.child_link)
    )

    # Test non-zero joint value
    transform = joint.compute_transform(1.0)
    half_angle = 1.0 / 2.0
    sin_half = math.sin(half_angle)
    cos_half = math.cos(half_angle)
    dynamic_quat = [axis_x * sin_half, axis_y * sin_half, axis_z * sin_half, cos_half]
    expected_quat = rru._quat_multiply(origin_quat, dynamic_quat)
    assert transform.translation == rr.components.Translation3DBatch(rr.components.Translation3D(joint.origin_xyz))
    assert transform.quaternion == rr.components.RotationQuatBatch(rr.components.RotationQuat(xyzw=expected_quat))
