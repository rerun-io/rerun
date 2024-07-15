from __future__ import annotations

import itertools
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
    Scale3D,
    Vec3D,
)

from .common_arrays import none_empty_or_value
from .test_matnxn import MAT_3X3_INPUT
from .test_vecnd import VEC_3D_INPUT, assert_correct_vec3d

SCALE_3D_INPUT = [
    # Uniform
    4,
    4.0,
    Fraction(8, 2),
    # ThreeD
    *VEC_3D_INPUT,
]


def assert_correct_scale3d(scale: Scale3D | None) -> None:
    assert scale is not None
    if isinstance(scale.inner, float):
        assert scale.inner == 4.0
    elif isinstance(scale.inner, Vec3D):
        assert_correct_vec3d(scale.inner)
    else:
        assert False, "Unexpected inner type"


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
        assert isinstance(rot.inner.angle.inner, float)
        assert rot.inner.angle.kind == "radians"

    else:
        assert False, f"Unexpected inner type: {type(rot.inner)}"


def test_angle() -> None:
    five_rad = [
        Angle(5),
        Angle(5.0),
        Angle(rad=5.0),
    ]

    for a in five_rad:
        assert a.inner == 5.0
        assert isinstance(a.inner, float)
        assert a.kind == "radians"

    five_deg = [
        Angle(deg=5),
        Angle(deg=5.0),
    ]

    for a in five_deg:
        assert a.inner == 5.0
        assert isinstance(a.inner, float)
        assert a.kind == "degrees"


def test_transform3d() -> None:
    axis_lengths = [None, 1, 1.0]
    from_parent_arrays = [None, True, False]

    # TODO(#6831): repopulate this list with all transform variants
    all_arrays = itertools.zip_longest(
        MAT_3X3_INPUT + [None],
        VEC_3D_INPUT + [None],
        from_parent_arrays,
        axis_lengths,
    )

    for (
        mat3x3,
        translation,
        from_parent,
        axis_length,
    ) in all_arrays:
        mat3x3 = cast(Optional[rr.datatypes.Mat3x3Like], mat3x3)
        translation = cast(Optional[rr.datatypes.Vec3DLike], translation)
        from_parent = cast(Optional[bool], from_parent)
        axis_length = cast(Optional[rr.datatypes.Float32Like], axis_length)

        print(
            f"rr.Transform3D(\n"
            f"    mat3x3={mat3x3!r}\n"  #
            f"    translation={translation!r}\n"  #
            f"    from_parent={from_parent!r}\n"  #
            f"    axis_length={axis_length!r}\n"  #
            f")"
        )
        arch = rr.Transform3D(
            mat3x3=mat3x3,
            translation=translation,
            from_parent=from_parent,
            axis_length=axis_length,
        )
        print(f"{arch}\n")

        assert arch.mat3x3 == rr.components.TransformMat3x3Batch._optional(
            none_empty_or_value(mat3x3, rr.components.TransformMat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]))
        )
        print("left")
        print(arch.translation)
        print("right")
        print(
            rr.components.Translation3DBatch._optional(
                none_empty_or_value(translation, rr.components.Translation3D([1, 2, 3]))
            )
        )
        assert arch.translation == rr.components.Translation3DBatch._optional(
            none_empty_or_value(translation, rr.components.Translation3D([1, 2, 3]))
        )
        assert arch.axis_length == rr.components.AxisLengthBatch._optional(
            none_empty_or_value(axis_length, rr.components.AxisLength(1.0))
        )
        # TODO(#6831): from parent!
        # assert arch.from_parent == rr.components.Bool._optional(none_empty_or_value(from_parent, False))


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
