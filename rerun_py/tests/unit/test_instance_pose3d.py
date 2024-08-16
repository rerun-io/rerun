from __future__ import annotations

import itertools
from fractions import Fraction
from typing import Optional, cast

import rerun as rr
from rerun.datatypes import (
    Quaternion,
    RotationAxisAngle,
)

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


def test_instance_poses3d() -> None:
    rotation_axis_angle_arrays = [
        None,
        RotationAxisAngle([1, 2, 3], rr.Angle(deg=10)),
        [RotationAxisAngle([1, 2, 3], rr.Angle(deg=10)), RotationAxisAngle([3, 2, 1], rr.Angle(rad=1))],
    ]
    quaternion_arrays = [
        None,
        Quaternion(xyzw=[1, 2, 3, 4]),
        [Quaternion(xyzw=[4, 3, 2, 1]), Quaternion(xyzw=[1, 2, 3, 4])],
    ]

    # TODO(andreas): It would be nice to support scalar values here
    scale_arrays = [None, [1.0, 2.0, 3.0], [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]]

    all_arrays = itertools.zip_longest(
        VEC_3D_INPUT + [None],
        rotation_axis_angle_arrays,
        quaternion_arrays,
        scale_arrays,
        MAT_3X3_INPUT + [None],
    )

    for (
        translation,
        rotation_axis_angle,
        quaternion,
        scale,
        mat3x3,
    ) in all_arrays:
        translations = cast(Optional[rr.datatypes.Vec3DArrayLike], translation)
        rotation_axis_angles = cast(Optional[rr.datatypes.RotationAxisAngleArrayLike], rotation_axis_angle)
        quaternions = cast(Optional[rr.datatypes.QuaternionArrayLike], quaternion)
        scales = cast(Optional[rr.datatypes.Vec3DArrayLike | rr.datatypes.Float32Like], scale)
        mat3x3 = cast(Optional[rr.datatypes.Mat3x3ArrayLike], mat3x3)

        print(
            f"rr.InstancePoses3D(\n"
            f"    translations={translations!r}\n"  #
            f"    rotation_axis_angles={rotation_axis_angles!r}\n"  #
            f"    quaternions={quaternions!r}\n"  #
            f"    scales={scales!r}\n"  #
            f"    mat3x3={mat3x3!r}\n"  #
            f")"
        )
        arch = rr.InstancePoses3D(
            translations=translations,
            rotation_axis_angles=rotation_axis_angles,
            quaternions=quaternions,
            scales=scales,
            mat3x3=mat3x3,
        )
        print(f"{arch}\n")

        assert arch.translations == rr.components.PoseTranslation3DBatch._optional(translations)
        assert arch.rotation_axis_angles == rr.components.PoseRotationAxisAngleBatch._optional(rotation_axis_angles)
        assert arch.quaternions == rr.components.PoseRotationQuatBatch._optional(quaternions)
        assert arch.scales == rr.components.PoseScale3DBatch._optional(scales)
        assert arch.mat3x3 == rr.components.PoseTransformMat3x3Batch._optional(mat3x3)
