from __future__ import annotations

import itertools
import math
from fractions import Fraction
from typing import cast

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.datatypes import (
    Angle,
    Float64ArrayLike,
    Quaternion,
    RotationAxisAngle,
    Vec3D,
)

from .common_arrays import none_empty_or_value
from .test_matnxn import MAT_3X3_INPUT
from .test_vecnd import VEC_3D_INPUT

SCALE_3D_INPUT = [
    # Uniform
    4,
    4.0,
    Fraction(8, 2),
    # ThreeD
    *VEC_3D_INPUT,
]

ROTATION_3D_INPUT = [
    # Quaternion
    [1, 2, 3, 4],
    [1.0, 2.0, 3.0, 4.0],
    np.array([1, 2, 3, 4]),
    torch.tensor([1, 2, 3, 4]),
    Quaternion(xyzw=[1, 2, 3, 4]),
    Quaternion(xyzw=[1.0, 2.0, 3.0, 4.0]),
    Quaternion(xyzw=np.array([1, 2, 3, 4])),
    # RotationAxisAngle
    RotationAxisAngle([1, 2, 3], 4),
    RotationAxisAngle([1.0, 2.0, 3.0], Angle(4)),
    RotationAxisAngle(Vec3D([1, 2, 3]), Angle(4)),
    RotationAxisAngle(np.array([1, 2, 3], dtype=np.uint8), Angle(rad=4)),
]


def test_angle() -> None:
    five_rad = [
        Angle(5),
        Angle(5.0),
        Angle(rad=5.0),
    ]

    for a in five_rad:
        assert a.radians == 5.0

    five_deg = [
        Angle(deg=5),
        Angle(deg=5.0),
    ]

    for a in five_deg:
        assert a.radians == math.radians(5.0)


def test_transform3d() -> None:
    rotation_axis_angle_original = [None, RotationAxisAngle([1, 2, 3], rr.Angle(deg=10))]
    quaternion_arrays = [None, Quaternion(xyzw=[1, 2, 3, 4])]
    scale_arrays = [None, 1.0, 1, [1.0, 2.0, 3.0], rr.Scale3D([1.0, 2.0, 3.0])]
    relations = [
        None,
        rr.TransformRelation.ParentFromChild,
        rr.TransformRelation.ChildFromParent,
        "parentfromchild",
        "childfromparent",
    ]

    all_arrays = itertools.zip_longest(
        [*VEC_3D_INPUT, None],
        rotation_axis_angle_original,
        quaternion_arrays,
        scale_arrays,
        [*MAT_3X3_INPUT, None],
        relations,
    )

    for (
        translation,
        rotation_axis_angle,
        quaternion,
        scale,
        mat3x3,
        relation,
    ) in all_arrays:
        translation = cast("rr.datatypes.Vec3DLike | None", translation)
        quaternion = cast("rr.datatypes.QuaternionLike | None", quaternion)
        scale = cast("rr.datatypes.Vec3DLike | None", scale)
        mat3x3 = cast("rr.datatypes.Mat3x3Like | None", mat3x3)
        relation = cast("rr.components.TransformRelationLike | None", relations)

        print(
            f"rr.Transform3D(\n"
            f"    translation={translation!r}\n"
            f"    rotation_axis_angle={rotation_axis_angle!r}\n"
            f"    quaternion={quaternion!r}\n"
            f"    scale={scale!r}\n"
            f"    mat3x3={mat3x3!r}\n"
            f"    relation={relation!r}\n"
            f")",
        )
        arch = rr.Transform3D(
            translation=translation,
            rotation_axis_angle=rotation_axis_angle,  # type: ignore[arg-type] # prior cast didn't work here
            quaternion=quaternion,
            scale=scale,
            mat3x3=mat3x3,
            relation=relation,
        )
        print(f"{arch}\n")

        assert arch.scale == none_empty_or_value(scale, rr.components.Scale3DBatch(rr.components.Scale3D(scale)))
        assert arch.rotation_axis_angle == none_empty_or_value(
            rotation_axis_angle,
            rr.components.RotationAxisAngleBatch(rr.components.RotationAxisAngle([1, 2, 3], Angle(deg=10))),
        )
        assert arch.quaternion == none_empty_or_value(
            quaternion,
            rr.components.RotationQuatBatch(rr.components.RotationQuat(xyzw=[1, 2, 3, 4])),
        )
        assert arch.translation == none_empty_or_value(
            translation,
            rr.components.Translation3DBatch(rr.components.Translation3D([1, 2, 3])),
        )
        assert arch.mat3x3 == none_empty_or_value(
            mat3x3,
            rr.components.TransformMat3x3Batch(rr.components.TransformMat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]])),
        )
        assert arch.relation == rr.components.TransformRelationBatch(relation)


def test_transform_axes_3d() -> None:
    axis_lengths = [1, 1.0]

    for axis_length in axis_lengths:
        axes = rr.TransformAxes3D(axis_length)
        assert axes.axis_length == rr.components.AxisLengthBatch(rr.components.AxisLength(axis_length))


def test_transform_mat3x3_snippets() -> None:
    np.testing.assert_array_equal(
        rr.components.TransformMat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
        np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.components.TransformMat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.components.TransformMat3x3(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )
    np.testing.assert_array_equal(
        rr.components.TransformMat3x3(columns=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
        np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
    )


def test_transform3d_rotation() -> None:
    assert rr.Transform3D(rotation=RotationAxisAngle([1, 2, 3], rr.Angle(deg=10))) == rr.Transform3D(
        rotation_axis_angle=RotationAxisAngle([1, 2, 3], rr.Angle(deg=10)),
    )
    assert rr.Transform3D(rotation=Quaternion(xyzw=[1, 2, 3, 4])) == rr.Transform3D(
        quaternion=Quaternion(xyzw=[1, 2, 3, 4]),
    )


TRANSLATION_CASES: list[tuple[Float64ArrayLike, Float64ArrayLike]] = [
    ([], []),
    (np.ones((10, 3)), np.ones((10, 1, 3)).tolist()),
    (np.zeros((5, 3)), np.zeros((5, 1, 3)).tolist()),
    (np.array([[1.0, 2.0, 3.0]]), [[[1.0, 2.0, 3.0]]]),
    (np.array([[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]), [[[0.0, 0.0, 0.0]], [[1.0, 1.0, 1.0]]]),
    (np.array([[5.5, -3.2, 0.0], [0.0, 10.7, -8.9]]), [[[5.5, -3.2, 0.0]], [[0.0, 10.7, -8.9]]]),
    (
        np.array([[-1.0, -2.0, -3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]),
        [[[-1.0, -2.0, -3.0]], [[4.0, 5.0, 6.0]], [[7.0, 8.0, 9.0]]],
    ),
    (np.zeros((0, 3)), []),
    (np.array([[1000.0, 2000.0, 3000.0]]), [[[1000.0, 2000.0, 3000.0]]]),
]


def test_transform3d_translation_columns() -> None:
    for input, expected in TRANSLATION_CASES:
        data = [*rr.Transform3D.columns(translation=input)]
        assert np.allclose(data[0].as_arrow_array().to_pylist(), np.asarray(expected))


MAT_3X3_CASES: list[tuple[Float64ArrayLike, Float64ArrayLike]] = [
    ([], []),
    (np.ones((10, 3, 3)), np.ones((10, 1, 9)).tolist()),
    (np.eye(3).reshape(1, 3, 3), [[[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]]]),
    (
        np.array([
            [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            [[0, 1, 0], [0, 0, 1], [1, 0, 0]],
        ]),
        [[[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]], [[0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]]],
    ),
    (np.zeros((5, 3, 3)), np.zeros((5, 1, 9)).tolist()),
    (np.array([[[-1, -2, -3], [-4, -5, -6], [-7, -8, -9]]]), [[[-1, -4, -7, -2, -5, -8, -3, -6, -9]]]),
    (
        np.array([[[1000, 2000, 3000], [4000, 5000, 6000], [7000, 8000, 9000]]]),
        [[[1000, 4000, 7000, 2000, 5000, 8000, 3000, 6000, 9000]]],
    ),
    (np.zeros((0, 3, 3)), []),
]


@pytest.mark.parametrize("matrix_input, matrix_expected", MAT_3X3_CASES)
def test_transform3d_mat3x3_columns(matrix_input: Float64ArrayLike, matrix_expected: Float64ArrayLike) -> None:
    print(matrix_input)
    data = [*rr.Transform3D.columns(mat3x3=matrix_input)]
    assert np.allclose(data[0].as_arrow_array().to_pylist(), np.asarray(matrix_expected))
