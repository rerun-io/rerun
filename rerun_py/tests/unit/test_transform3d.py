from __future__ import annotations

from fractions import Fraction

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components import Transform3D, Transform3DBatch
from rerun.datatypes import (
    Angle,
    Mat3x3,
    Mat3x3Like,
    Quaternion,
    Rotation3D,
    Rotation3DLike,
    RotationAxisAngle,
    Scale3D,
    Scale3DLike,
    TranslationAndMat3x3,
    TranslationRotationScale3D,
    Vec3D,
    Vec3DLike,
)

from .test_matnxn import MAT_3X3_INPUT, assert_correct_mat3x3
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


@pytest.mark.parametrize("input", SCALE_3D_INPUT)
def test_scale3d(input: Scale3DLike) -> None:
    assert_correct_scale3d(Scale3D(input))


@pytest.mark.parametrize("input", ROTATION_3D_INPUT)
def test_rotation3d(input: Rotation3DLike) -> None:
    assert_correct_rotation3d(Rotation3D(input))


@pytest.mark.parametrize("input", VEC_3D_INPUT)
def test_translation_rotation_translation(input: Vec3DLike) -> None:
    trs = TranslationRotationScale3D(translation=input)
    assert_correct_vec3d(trs.translation)


@pytest.mark.parametrize("input", SCALE_3D_INPUT)
def test_translation_rotation_scale(input: Scale3DLike) -> None:
    trs = TranslationRotationScale3D(scale=input)
    assert_correct_scale3d(trs.scale)

    trs = TranslationRotationScale3D(scale=Scale3D(input))
    assert_correct_scale3d(trs.scale)


@pytest.mark.parametrize("input", ROTATION_3D_INPUT)
def test_translation_rotation_rotation(input: Rotation3DLike) -> None:
    trs = TranslationRotationScale3D(rotation=input)
    assert_correct_rotation3d(trs.rotation)

    trs = TranslationRotationScale3D(rotation=Rotation3D(input))
    assert_correct_rotation3d(trs.rotation)


def test_translation_rotation_from_parent() -> None:
    assert not TranslationRotationScale3D().from_parent
    assert TranslationRotationScale3D(from_parent=True).from_parent
    assert not TranslationRotationScale3D(from_parent=False).from_parent


@pytest.mark.parametrize("trans", VEC_3D_INPUT + [None])
@pytest.mark.parametrize("mat", MAT_3X3_INPUT + [None])
def test_translation_and_mat3x3(trans: Vec3DLike | None, mat: Mat3x3Like | None) -> None:
    tm = TranslationAndMat3x3(translation=trans, mat3x3=mat)
    if trans is None:
        assert tm.translation is None
    else:
        assert_correct_vec3d(tm.translation)
    if mat is None:
        assert tm.mat3x3 is None
    else:
        assert_correct_mat3x3(tm.mat3x3)


def test_translation_and_mat3x3_from_parent() -> None:
    assert not TranslationAndMat3x3().from_parent
    assert TranslationAndMat3x3(from_parent=True).from_parent
    assert not TranslationAndMat3x3(from_parent=False).from_parent


# SERIALIZATION TESTS
# This should cover all acceptable input to the Transform3D archetype


@pytest.mark.parametrize("trans", VEC_3D_INPUT + [None])
@pytest.mark.parametrize("mat", MAT_3X3_INPUT + [None])
def test_transform3d_translation_and_mat3x3(trans: Vec3DLike | None, mat: Mat3x3Like | None) -> None:
    expected_trans = Vec3D([1, 2, 3]) if trans is not None else None
    expected_mat = Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]) if mat is not None else None

    tm = rr.Transform3D(TranslationAndMat3x3(translation=trans, mat3x3=mat))

    assert tm.transform == Transform3DBatch(
        Transform3D(
            TranslationAndMat3x3(
                translation=expected_trans,
                mat3x3=expected_mat,
            )
        )
    )
    if mat is None:
        assert rr.Transform3D(TranslationRotationScale3D(translation=trans)) == rr.Transform3D(
            translation=trans,
            mat3x3=mat,
        )
    else:
        assert tm == rr.Transform3D(
            translation=trans,
            mat3x3=mat,
        )

    tm2 = rr.Transform3D(TranslationAndMat3x3(translation=trans, mat3x3=mat, from_parent=True))

    assert tm2.transform == Transform3DBatch(
        Transform3D(
            TranslationAndMat3x3(
                translation=expected_trans,
                mat3x3=expected_mat,
                from_parent=True,
            )
        )
    )
    if mat is None:
        assert rr.Transform3D(TranslationRotationScale3D(translation=trans, from_parent=True)) == rr.Transform3D(
            translation=trans,
            mat3x3=mat,
            from_parent=True,
        )
    else:
        assert tm2 == rr.Transform3D(
            translation=expected_trans,
            mat3x3=expected_mat,
            from_parent=True,
        )

    assert tm != tm2


@pytest.mark.parametrize("trans", VEC_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_translation(trans: Vec3DLike) -> None:
    tm = rr.Transform3D(TranslationRotationScale3D(translation=trans))

    assert tm.transform == Transform3DBatch(Transform3D(TranslationRotationScale3D(translation=Vec3D([1, 2, 3]))))
    assert tm == rr.Transform3D(translation=Vec3D([1, 2, 3]))

    tm2 = rr.Transform3D(TranslationRotationScale3D(translation=trans, from_parent=True))

    assert tm2.transform == Transform3DBatch(
        Transform3D(TranslationRotationScale3D(translation=Vec3D([1, 2, 3]), from_parent=True))
    )
    assert tm2 == rr.Transform3D(translation=Vec3D([1, 2, 3]), from_parent=True)

    assert tm2 != tm


@pytest.mark.parametrize("rot", ROTATION_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_rotation(rot: Rotation3DLike) -> None:
    tm = rr.Transform3D(TranslationRotationScale3D(rotation=rot))

    assert tm.transform == Transform3DBatch(
        Transform3D(TranslationRotationScale3D(rotation=Rotation3D(Quaternion(xyzw=[1, 2, 3, 4]))))
    ) or tm.transform == Transform3DBatch(
        Transform3D(TranslationRotationScale3D(rotation=Rotation3D(RotationAxisAngle(Vec3D([1, 2, 3]), Angle(rad=4)))))
    )

    assert tm == rr.Transform3D(rotation=Rotation3D(Quaternion(xyzw=[1, 2, 3, 4]))) or tm == rr.Transform3D(
        rotation=Rotation3D(RotationAxisAngle(Vec3D([1, 2, 3]), Angle(rad=4)))
    )


@pytest.mark.parametrize("scale", SCALE_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_scale(scale: Scale3DLike) -> None:
    tm = rr.Transform3D(TranslationRotationScale3D(scale=scale))

    assert tm.transform == Transform3DBatch(
        Transform3D(TranslationRotationScale3D(scale=Scale3D([1, 2, 3])))
    ) or tm.transform == Transform3DBatch(Transform3D(TranslationRotationScale3D(scale=Scale3D(4.0))))
    assert tm == rr.Transform3D(scale=Scale3D([1, 2, 3])) or tm == rr.Transform3D(scale=Scale3D(4.0))


def test_transform3d_invalid_parameter_combinations() -> None:
    rr.set_strict_mode(True)

    # combine transform with anything else.
    with pytest.raises(ValueError):
        rr.Transform3D(transform=TranslationRotationScale3D(translation=[1, 2, 3]), translation=[1, 2, 3])
    with pytest.raises(ValueError):
        rr.Transform3D(transform=TranslationRotationScale3D(translation=[1, 2, 3]), scale=2)
    with pytest.raises(ValueError):
        rr.Transform3D(transform=TranslationRotationScale3D(translation=[1, 2, 3]), from_parent=True)
    with pytest.raises(ValueError):
        rr.Transform3D(
            transform=TranslationRotationScale3D(translation=[1, 2, 3]), rotation=rr.Quaternion(xyzw=[1, 2, 3, 4])
        )
    with pytest.raises(ValueError):
        rr.Transform3D(transform=TranslationRotationScale3D(translation=[1, 2, 3]), mat3x3=[1, 2, 3, 4, 5, 6, 7, 8, 9])

    # combine 3x3 matrix with rotation or scale
    with pytest.raises(ValueError):
        rr.Transform3D(mat3x3=[1, 2, 3, 4, 5, 6, 7, 8, 9], scale=2)
    with pytest.raises(ValueError):
        rr.Transform3D(mat3x3=[1, 2, 3, 4, 5, 6, 7, 8, 9], rotation=rr.Quaternion(xyzw=[1, 2, 3, 4]))
