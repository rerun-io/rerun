from __future__ import annotations

import math
from pathlib import Path

import numpy as np
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

    clamped_angle = joint.limit_upper + 1.0
    with pytest.warns(UserWarning, match="outside limits"):
        transform = joint.compute_transform(clamped_angle)

    expected_angle = joint.limit_upper
    origin_roll, origin_pitch, origin_yaw = joint.origin_rpy
    expected_matrix = rpy_matrix(origin_roll, origin_pitch, origin_yaw) @ axis_angle_matrix(joint.axis, expected_angle)
    expected_quat = quat_from_matrix(expected_matrix)

    assert_translation_expected(transform.translation, joint.origin_xyz)
    assert_quat_equivalent(transform.quaternion, expected_quat)
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.parent_link)
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.child_link)
    )


def rot_x(angle: float) -> np.ndarray:
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    return np.array(
        [
            [1.0, 0.0, 0.0],
            [0.0, cos_a, -sin_a],
            [0.0, sin_a, cos_a],
        ],
        dtype=float,
    )


def rot_y(angle: float) -> np.ndarray:
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    return np.array(
        [
            [cos_a, 0.0, sin_a],
            [0.0, 1.0, 0.0],
            [-sin_a, 0.0, cos_a],
        ],
        dtype=float,
    )


def rot_z(angle: float) -> np.ndarray:
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    return np.array(
        [
            [cos_a, -sin_a, 0.0],
            [sin_a, cos_a, 0.0],
            [0.0, 0.0, 1.0],
        ],
        dtype=float,
    )


def rpy_matrix(roll: float, pitch: float, yaw: float) -> np.ndarray:
    return rot_z(yaw) @ rot_y(pitch) @ rot_x(roll)


def axis_angle_matrix(axis: tuple[float, float, float], angle: float) -> np.ndarray:
    axis_x, axis_y, axis_z = axis
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    one_minus = 1.0 - cos_a
    return np.array(
        [
            [
                cos_a + axis_x * axis_x * one_minus,
                axis_x * axis_y * one_minus - axis_z * sin_a,
                axis_x * axis_z * one_minus + axis_y * sin_a,
            ],
            [
                axis_y * axis_x * one_minus + axis_z * sin_a,
                cos_a + axis_y * axis_y * one_minus,
                axis_y * axis_z * one_minus - axis_x * sin_a,
            ],
            [
                axis_z * axis_x * one_minus - axis_y * sin_a,
                axis_z * axis_y * one_minus + axis_x * sin_a,
                cos_a + axis_z * axis_z * one_minus,
            ],
        ],
        dtype=float,
    )


def quat_from_matrix(matrix: np.ndarray) -> list[float]:
    trace = matrix[0, 0] + matrix[1, 1] + matrix[2, 2]
    if trace > 0.0:
        scale = math.sqrt(trace + 1.0) * 2.0
        w = 0.25 * scale
        x = (matrix[2, 1] - matrix[1, 2]) / scale
        y = (matrix[0, 2] - matrix[2, 0]) / scale
        z = (matrix[1, 0] - matrix[0, 1]) / scale
    elif matrix[0, 0] > matrix[1, 1] and matrix[0, 0] > matrix[2, 2]:
        scale = math.sqrt(1.0 + matrix[0, 0] - matrix[1, 1] - matrix[2, 2]) * 2.0
        w = (matrix[2, 1] - matrix[1, 2]) / scale
        x = 0.25 * scale
        y = (matrix[0, 1] + matrix[1, 0]) / scale
        z = (matrix[0, 2] + matrix[2, 0]) / scale
    elif matrix[1, 1] > matrix[2, 2]:
        scale = math.sqrt(1.0 + matrix[1, 1] - matrix[0, 0] - matrix[2, 2]) * 2.0
        w = (matrix[0, 2] - matrix[2, 0]) / scale
        x = (matrix[0, 1] + matrix[1, 0]) / scale
        y = 0.25 * scale
        z = (matrix[1, 2] + matrix[2, 1]) / scale
    else:
        scale = math.sqrt(1.0 + matrix[2, 2] - matrix[0, 0] - matrix[1, 1]) * 2.0
        w = (matrix[1, 0] - matrix[0, 1]) / scale
        x = (matrix[0, 2] + matrix[2, 0]) / scale
        y = (matrix[1, 2] + matrix[2, 1]) / scale
        z = 0.25 * scale
    return [x, y, z, w]


def assert_quat_equivalent(actual: rr.components.RotationQuatBatch, expected: list[float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    dot = sum(a * b for a, b in zip(actual_values, expected, strict=False))
    if dot < 0.0:
        expected = [-value for value in expected]
    assert actual_values == pytest.approx(expected)


def assert_translation_expected(actual: rr.components.Translation3DBatch, expected: tuple[float, float, float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    assert actual_values == pytest.approx(expected)
