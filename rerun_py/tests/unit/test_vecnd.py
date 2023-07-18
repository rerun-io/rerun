from __future__ import annotations

from fractions import Fraction

import numpy as np
import pytest
from rerun.experimental import dt as rr_dt

VEC_2D_INPUT = [
    [1, 2],
    [1.0, 2.0],
    [Fraction(1, 1), Fraction(2, 1)],
    rr_dt.Vec3D([1, 2]),
    np.array([1, 2]),
    np.array([1.0, 2.0]),
]


def assert_correct_vec2d(vec: rr_dt.Vec2D | None) -> None:
    assert vec is not None
    assert np.all(vec.xy == np.array([1.0, 2.0]))
    assert vec.xy.dtype == np.float32


VEC_3D_INPUT = [
    [1, 2, 3],
    [1.0, 2.0, 3.0],
    [Fraction(1, 1), Fraction(2, 1), Fraction(3, 1)],
    rr_dt.Vec3D([1, 2, 3]),
    np.array([1, 2, 3]),
    np.array([1.0, 2.0, 3.0]),
]


def assert_correct_vec3d(vec: rr_dt.Vec3D | None) -> None:
    assert vec is not None
    assert np.all(vec.xyz == np.array([1.0, 2.0, 3.0]))
    assert vec.xyz.dtype == np.float32


VEC_4D_INPUT = [
    [1, 2, 3, 4],
    [1.0, 2.0, 3.0, 4.0],
    [Fraction(1, 1), Fraction(2, 1), Fraction(3, 1), Fraction(4, 1)],
    rr_dt.Vec4D([1, 2, 3, 4]),
    np.array([1, 2, 3, 4]),
    np.array([1.0, 2.0, 3.0, 4.0]),
]


def assert_correct_vec4d(vec: rr_dt.Vec4D | None) -> None:
    assert vec is not None
    assert np.all(vec.xyzw == np.array([1.0, 2.0, 3.0, 4.0]))
    assert vec.xyzw.dtype == np.float32


@pytest.mark.parametrize("data", VEC_2D_INPUT)
def test_vec2d(data: rr_dt.Vec2DLike) -> None:
    vec = rr_dt.Vec2D(data)
    assert_correct_vec2d(vec)


@pytest.mark.parametrize("data", VEC_3D_INPUT)
def test_vec3d(data: rr_dt.Vec3DLike) -> None:
    vec = rr_dt.Vec3D(data)
    assert_correct_vec3d(vec)


@pytest.mark.parametrize("data", VEC_4D_INPUT)
def test_vec4d(data: rr_dt.Vec4DLike) -> None:
    vec = rr_dt.Vec4D(data)
    assert_correct_vec4d(vec)
