from __future__ import annotations

import itertools
import math
from fractions import Fraction
from typing import Optional, cast

import numpy as np
import rerun as rr
import torch
from rerun.datatypes import (
    Angle,
    Quaternion,
    Rotation3D,
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


def assert_correct_rotation3d(rot: Rotation3D | None) -> None:
    assert rot is not None
    if isinstance(rot.inner, Quaternion):
        assert np.all(rot.inner.xyzw == np.array([1.0, 2.0, 3.0, 4.0]))
        assert rot.inner.xyzw.dtype == np.float32
    elif isinstance(rot.inner, RotationAxisAngle):
        # TODO(#2650): np.array-typed fields should be provided with a `eq` method that uses `np.all`
        assert np.all(rot.inner.axis.xyz == np.array([1.0, 2.0, 3.0]))
        assert rot.inner.axis.xyz.dtype == np.float32
        assert rot.inner.angle == Angle(4.0)

    else:
        assert False, f"Unexpected inner type: {type(rot.inner)}"


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
    rotation_axis_angle = [None, RotationAxisAngle([1, 2, 3], rr.Angle(deg=10))]
    quaternion_arrays = [None, Quaternion(xyzw=[1, 2, 3, 4])]
    scale_arrays = [None, 1.0, 1, [1.0, 2.0, 3.0]]
    axis_lengths = [None, 1, 1.0]
    relations = [
        None,
        rr.TransformRelation.ParentFromChild,
        rr.TransformRelation.ChildFromParent,
        "parentfromchild",
        "childfromparent",
    ]

    # TODO(#6831): repopulate this list with all transform variants
    all_arrays = itertools.zip_longest(
        VEC_3D_INPUT + [None],
        rotation_axis_angle,
        quaternion_arrays,
        scale_arrays,
        MAT_3X3_INPUT + [None],
        relations,
        axis_lengths,
    )

    for (
        translation,
        rotation_axis_angle,
        quaternion,
        scale,
        mat3x3,
        relation,
        axis_length,
    ) in all_arrays:
        translation = cast(Optional[rr.datatypes.Vec3DLike], translation)
        quaternion = cast(Optional[rr.datatypes.QuaternionLike], quaternion)
        scale = cast(Optional[rr.datatypes.Vec3DLike | rr.datatypes.Float32Like], scale)
        mat3x3 = cast(Optional[rr.datatypes.Mat3x3Like], mat3x3)
        relation = cast(Optional[rr.components.TransformRelationLike], relations)
        axis_length = cast(Optional[rr.datatypes.Float32Like], axis_length)

        print(
            f"rr.Transform3D(\n"
            f"    translation={translation!r}\n"  #
            f"    rotation_axis_angle={rotation_axis_angle!r}\n"  #
            f"    quaternion={quaternion!r}\n"  #
            f"    scale={scale!r}\n"  #
            f"    mat3x3={mat3x3!r}\n"  #
            f"    relation={relation!r}\n"  #
            f"    axis_length={axis_length!r}\n"  #
            f")"
        )
        arch = rr.Transform3D(
            translation=translation,
            rotation_axis_angle=rotation_axis_angle,  # type: ignore[assignment, arg-type] # prior cast didn't work here
            quaternion=quaternion,
            scale=scale,
            mat3x3=mat3x3,
            relation=relation,
            axis_length=axis_length,
        )
        print(f"{arch}\n")

        assert arch.scale == rr.components.Scale3DBatch._optional(
            none_empty_or_value(scale, rr.components.Scale3D(scale))  # type: ignore[arg-type]
        )
        assert arch.rotation_axis_angle == rr.components.RotationAxisAngleBatch._optional(
            none_empty_or_value(rotation_axis_angle, rr.components.RotationAxisAngle([1, 2, 3], Angle(deg=10)))
        )
        assert arch.quaternion == rr.components.RotationQuatBatch._optional(
            none_empty_or_value(quaternion, rr.components.RotationQuat(xyzw=[1, 2, 3, 4]))
        )
        assert arch.translation == rr.components.Translation3DBatch._optional(
            none_empty_or_value(translation, rr.components.Translation3D([1, 2, 3]))
        )
        assert arch.mat3x3 == rr.components.TransformMat3x3Batch._optional(
            none_empty_or_value(mat3x3, rr.components.TransformMat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]))
        )
        assert arch.axis_length == rr.components.AxisLengthBatch._optional(
            none_empty_or_value(axis_length, rr.components.AxisLength(1.0))
        )
        assert arch.relation == rr.components.TransformRelationBatch._optional(relation)


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
        rotation_axis_angle=RotationAxisAngle([1, 2, 3], rr.Angle(deg=10))
    )
    assert rr.Transform3D(rotation=Quaternion(xyzw=[1, 2, 3, 4])) == rr.Transform3D(
        quaternion=Quaternion(xyzw=[1, 2, 3, 4])
    )
