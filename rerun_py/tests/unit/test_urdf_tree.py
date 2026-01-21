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

    # We expect flat, neigbouring paths for visual and collision geometries of links,
    # queryable by either link name or link object.
    visual_paths = tree.get_visual_geometry_paths(child_link)
    assert visual_paths[0] == "/so_arm100/visual_geometries/shoulder/visual_0"
    visual_paths = tree.get_visual_geometry_paths("wrist")
    assert visual_paths[0] == "/so_arm100/visual_geometries/wrist/visual_0"

    collision_paths = tree.get_collision_geometry_paths(child_link)
    assert collision_paths[0] == "/so_arm100/collision_geometries/shoulder/collision_0"
    collision_paths = tree.get_collision_geometry_paths("wrist")
    assert collision_paths[0] == "/so_arm100/collision_geometries/wrist/collision_0"

    root_link = tree.root_link()
    assert root_link.name == "base"


def test_urdf_tree_transform() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    clamped_angle = joint.limit_upper + 1.0
    with pytest.warns(UserWarning, match="outside limits"):
        transform = joint.compute_transform(clamped_angle)

    expected_angle = joint.limit_upper
    origin_roll, origin_pitch, origin_yaw = joint.origin_rpy
    quat_origin = rru._euler_to_quat(origin_roll, origin_pitch, origin_yaw)
    axis_x, axis_y, axis_z = joint.axis
    half_angle = expected_angle / 2.0
    sin_half = math.sin(half_angle)
    cos_half = math.cos(half_angle)
    quat_dynamic = [
        axis_x * sin_half,
        axis_y * sin_half,
        axis_z * sin_half,
        cos_half,
    ]
    expected_quat = rru._quat_multiply(quat_origin, quat_dynamic)

    assert transform.translation is not None
    assert transform.quaternion is not None
    assert_translation_expected(transform.translation, joint.origin_xyz)
    assert_quat_equivalent(transform.quaternion, expected_quat)
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.parent_link)
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.child_link)
    )


def assert_quat_equivalent(actual: rr.components.RotationQuatBatch, expected: list[float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    dot = sum(a * b for a, b in zip(actual_values, expected, strict=False))
    if dot < 0.0:
        expected = [-value for value in expected]
    assert actual_values == pytest.approx(expected)


def assert_translation_expected(actual: rr.components.Translation3DBatch, expected: tuple[float, float, float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    assert actual_values == pytest.approx(expected)
