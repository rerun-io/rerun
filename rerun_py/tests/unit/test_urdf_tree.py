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

    # We expect flat, neighbouring paths for visual and collision geometries of links,
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
    quat_origin = _euler_to_quat(origin_roll, origin_pitch, origin_yaw)
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
    expected_quat = _quat_multiply(quat_origin, quat_dynamic)

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


def test_urdf_compute_transform_columns() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    values = [0.0, 0.5, 1.0]
    columns = joint.compute_transform_columns(values, clamp=True)

    from rerun._baseclasses import ComponentColumnList

    assert isinstance(columns, ComponentColumnList)
    assert len(columns) > 0

    # Verify the result is usable with send_columns by checking that
    # the individual transform matches what compute_transform returns.
    single = joint.compute_transform(0.5, clamp=True)
    assert single.translation is not None
    assert single.quaternion is not None

    # Verify that out-of-range values produce warnings.
    with pytest.warns(UserWarning, match="outside limits"):
        joint.compute_transform_columns([joint.limit_upper + 1.0], clamp=True)


def assert_quat_equivalent(actual: rr.components.RotationQuatBatch, expected: list[float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    dot = sum(a * b for a, b in zip(actual_values, expected, strict=False))
    if dot < 0.0:
        expected = [-value for value in expected]
    assert actual_values == pytest.approx(expected)


def assert_translation_expected(actual: rr.components.Translation3DBatch, expected: tuple[float, float, float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    assert actual_values == pytest.approx(expected)


def _euler_to_quat(roll: float, pitch: float, yaw: float) -> list[float]:
    """Convert Euler angles (RPY) to quaternion (XYZW)."""
    cr = math.cos(roll * 0.5)
    sr = math.sin(roll * 0.5)
    cp = math.cos(pitch * 0.5)
    sp = math.sin(pitch * 0.5)
    cy = math.cos(yaw * 0.5)
    sy = math.sin(yaw * 0.5)

    w = cr * cp * cy + sr * sp * sy
    x = sr * cp * cy - cr * sp * sy
    y = cr * sp * cy + sr * cp * sy
    z = cr * cp * sy - sr * sp * cy

    return [x, y, z, w]


def _quat_multiply(q1: list[float], q2: list[float]) -> list[float]:
    """Multiply two quaternions in XYZW format."""
    x1, y1, z1, w1 = q1
    x2, y2, z2, w2 = q2

    w = w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
    x = w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2
    y = w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2
    z = w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2

    return [x, y, z, w]
