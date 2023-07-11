from __future__ import annotations

from fractions import Fraction

import numpy as np
import pytest
import rerun as rr

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


def assert_correct_scale3d(scale: rr.dt.Scale3D | None) -> None:
    assert scale is not None
    if isinstance(scale.inner, float):
        assert scale.inner == 4.0
    elif isinstance(scale.inner, rr.dt.Vec3D):
        assert_correct_vec3d(scale.inner)
    else:
        assert False, "Unexpected inner type"


ROTATION_3D_INPUT = [
    # Quaternion
    [1, 2, 3, 4],
    [1.0, 2.0, 3.0, 4.0],
    np.array([1, 2, 3, 4]),
    rr.dt.Quaternion([1, 2, 3, 4]),
    rr.dt.Quaternion([1.0, 2.0, 3.0, 4.0]),
    rr.dt.Quaternion(np.array([1, 2, 3, 4])),
    # RotationAxisAngle
    rr.dt.RotationAxisAngle([1, 2, 3], 4),
    rr.dt.RotationAxisAngle([1.0, 2.0, 3.0], rr.dt.Angle(4)),
    rr.dt.RotationAxisAngle(rr.dt.Vec3D([1, 2, 3]), rr.dt.Angle(4)),
    rr.dt.RotationAxisAngle(np.array([1, 2, 3], dtype=np.uint8), rr.dt.Angle(rad=4)),
]


def assert_correct_rotation3d(rot: rr.dt.Rotation3D | None) -> None:
    assert rot is not None
    if isinstance(rot.inner, rr.dt.Quaternion):
        assert np.all(rot.inner.xyzw == np.array([1.0, 2.0, 3.0, 4.0]))
        assert rot.inner.xyzw.dtype == np.float32
    elif isinstance(rot.inner, rr.dt.RotationAxisAngle):
        # TODO(#2650): np.array-typed fields should be provided with a `eq` method that uses `np.all`
        assert np.all(rot.inner.axis.xyz == np.array([1.0, 2.0, 3.0]))
        assert rot.inner.axis.xyz.dtype == np.float32
        assert rot.inner.angle == rr.dt.Angle(4.0)
        assert isinstance(rot.inner.angle.inner, float)
        assert rot.inner.angle.kind == "radians"

    else:
        assert False, f"Unexpected inner type: {type(rot.inner)}"


def test_angle() -> None:
    five_rad = [
        rr.dt.Angle(5),
        rr.dt.Angle(5.0),
        rr.dt.Angle(rad=5.0),
    ]

    for a in five_rad:
        assert a.inner == 5.0
        assert isinstance(a.inner, float)
        assert a.kind == "radians"

    five_deg = [
        rr.dt.Angle(deg=5),
        rr.dt.Angle(deg=5.0),
    ]

    for a in five_deg:
        assert a.inner == 5.0
        assert isinstance(a.inner, float)
        assert a.kind == "degrees"


@pytest.mark.parametrize("input", SCALE_3D_INPUT)
def test_scale3d(input: rr.dt.Scale3DLike) -> None:
    assert_correct_scale3d(rr.dt.Scale3D(input))


@pytest.mark.parametrize("input", ROTATION_3D_INPUT)
def test_rotation3d(input: rr.dt.Rotation3DLike) -> None:
    assert_correct_rotation3d(rr.dt.Rotation3D(input))


@pytest.mark.parametrize("input", VEC_3D_INPUT)
def test_translation_rotation_translation(input: rr.dt.Vec3DLike) -> None:
    trs = rr.dt.TranslationRotationScale3D(translation=input)
    assert_correct_vec3d(trs.translation)


@pytest.mark.parametrize("input", SCALE_3D_INPUT)
def test_translation_rotation_scale(input: rr.dt.Scale3DLike) -> None:
    trs = rr.dt.TranslationRotationScale3D(scale=input)
    assert_correct_scale3d(trs.scale)

    trs = rr.dt.TranslationRotationScale3D(scale=rr.dt.Scale3D(input))
    assert_correct_scale3d(trs.scale)


@pytest.mark.parametrize("input", ROTATION_3D_INPUT)
def test_translation_rotation_rotation(input: rr.dt.Rotation3DLike) -> None:
    trs = rr.dt.TranslationRotationScale3D(rotation=input)
    assert_correct_rotation3d(trs.rotation)

    trs = rr.dt.TranslationRotationScale3D(rotation=rr.dt.Rotation3D(input))
    assert_correct_rotation3d(trs.rotation)


def test_translation_rotation_from_parent() -> None:
    assert not rr.dt.TranslationRotationScale3D().from_parent
    assert rr.dt.TranslationRotationScale3D(from_parent=True).from_parent
    assert not rr.dt.TranslationRotationScale3D(from_parent=False).from_parent


@pytest.mark.parametrize("trans", VEC_3D_INPUT + [None])
@pytest.mark.parametrize("mat", MAT_3X3_INPUT + [None])
def test_translation_and_mat3x3(trans: rr.dt.Vec3DLike | None, mat: rr.dt.Mat3x3Like | None) -> None:
    tm = rr.dt.TranslationAndMat3x3(translation=trans, matrix=mat)
    if trans is None:
        assert tm.translation is None
    else:
        assert_correct_vec3d(tm.translation)
    if mat is None:
        assert tm.matrix is None
    else:
        assert_correct_mat3x3(tm.matrix)


def test_translation_and_mat3x3_from_parent() -> None:
    assert not rr.dt.TranslationAndMat3x3().from_parent
    assert rr.dt.TranslationAndMat3x3(from_parent=True).from_parent
    assert not rr.dt.TranslationAndMat3x3(from_parent=False).from_parent


# SERIALISATION TESTS
# This should cover all acceptable input to the Transform3D archetype


@pytest.mark.parametrize("trans", VEC_3D_INPUT)
@pytest.mark.parametrize("mat", MAT_3X3_INPUT)
def test_transform3d_translation_and_mat3x3(trans: rr.dt.Vec3DLike, mat: rr.dt.Mat3x3Like) -> None:
    tm = rr.arch.Transform3D(rr.dt.TranslationAndMat3x3(translation=trans, matrix=mat))

    assert tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(
            rr.dt.TranslationAndMat3x3(
                translation=rr.dt.Vec3D([1, 2, 3]), matrix=rr.dt.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9])
            )
        )
    )

    tm2 = rr.arch.Transform3D(rr.dt.TranslationAndMat3x3(translation=trans, matrix=mat, from_parent=True))

    assert tm2.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(
            rr.dt.TranslationAndMat3x3(
                translation=rr.dt.Vec3D([1, 2, 3]),
                matrix=rr.dt.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]),
                from_parent=True,
            )
        )
    )

    assert tm != tm2


@pytest.mark.parametrize("trans", VEC_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_translation(trans: rr.dt.Vec3DLike) -> None:
    tm = rr.arch.Transform3D(rr.dt.TranslationRotationScale3D(translation=trans))

    assert tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(rr.dt.TranslationRotationScale3D(translation=rr.dt.Vec3D([1, 2, 3])))
    )

    tm2 = rr.arch.Transform3D(rr.dt.TranslationRotationScale3D(translation=trans, from_parent=True))

    assert tm2.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(rr.dt.TranslationRotationScale3D(translation=rr.dt.Vec3D([1, 2, 3]), from_parent=True))
    )

    assert tm2 != tm


@pytest.mark.parametrize("rot", ROTATION_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_rotation(rot: rr.dt.Rotation3DLike) -> None:
    tm = rr.arch.Transform3D(rr.dt.TranslationRotationScale3D(rotation=rot))

    assert tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(rr.dt.TranslationRotationScale3D(rotation=rr.dt.Rotation3D(rr.dt.Quaternion([1, 2, 3, 4]))))
    ) or tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(
            rr.dt.TranslationRotationScale3D(
                rotation=rr.dt.Rotation3D(rr.dt.RotationAxisAngle(rr.dt.Vec3D([1, 2, 3]), rr.dt.Angle(rad=4)))
            )
        )
    )


@pytest.mark.parametrize("scale", SCALE_3D_INPUT)
def test_transform3d_translation_rotation_scale3d_scale(scale: rr.dt.Scale3DLike) -> None:
    tm = rr.arch.Transform3D(rr.dt.TranslationRotationScale3D(scale=scale))

    assert tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(rr.dt.TranslationRotationScale3D(scale=rr.dt.Scale3D([1, 2, 3])))
    ) or tm.transform == rr.cmp.Transform3DArray.from_similar(
        rr.dt.Transform3D(rr.dt.TranslationRotationScale3D(scale=rr.dt.Scale3D(4.0)))
    )
