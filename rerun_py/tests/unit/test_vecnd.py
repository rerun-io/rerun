from __future__ import annotations

from fractions import Fraction

import numpy as np
import pytest
from rerun.datatypes import (
    Vec2D,
    Vec2DArrayLike,
    Vec2DBatch,
    Vec2DLike,
    Vec3D,
    Vec3DArrayLike,
    Vec3DBatch,
    Vec3DLike,
    Vec4D,
    Vec4DArrayLike,
    Vec4DBatch,
    Vec4DLike,
)

from .common_arrays import (
    vec2ds_arrays,
    vec2ds_expected,
    vec3ds_arrays,
    vec3ds_expected,
    vec4ds_arrays,
    vec4ds_expected,
)

VEC_2D_INPUT = [
    [1, 2],
    [1.0, 2.0],
    [Fraction(1, 1), Fraction(2, 1)],
    Vec3D([1, 2]),
    np.array([1, 2]),
    np.array([1.0, 2.0]),
]


def assert_correct_vec2d(vec: Vec2D | None) -> None:
    assert vec is not None
    assert np.all(vec.xy == np.array([1.0, 2.0]))
    assert vec.xy.dtype == np.float32


VEC_3D_INPUT = [
    [1, 2, 3],
    [1.0, 2.0, 3.0],
    [Fraction(1, 1), Fraction(2, 1), Fraction(3, 1)],
    Vec3D([1, 2, 3]),
    np.array([1, 2, 3]),
    np.array([1.0, 2.0, 3.0]),
]


def assert_correct_vec3d(vec: Vec3D | None) -> None:
    assert vec is not None
    assert np.all(vec.xyz == np.array([1.0, 2.0, 3.0]))
    assert vec.xyz.dtype == np.float32


VEC_4D_INPUT = [
    [1, 2, 3, 4],
    [1.0, 2.0, 3.0, 4.0],
    [Fraction(1, 1), Fraction(2, 1), Fraction(3, 1), Fraction(4, 1)],
    Vec4D([1, 2, 3, 4]),
    np.array([1, 2, 3, 4]),
    np.array([1.0, 2.0, 3.0, 4.0]),
]


def assert_correct_vec4d(vec: Vec4D | None) -> None:
    assert vec is not None
    assert np.all(vec.xyzw == np.array([1.0, 2.0, 3.0, 4.0]))
    assert vec.xyzw.dtype == np.float32


@pytest.mark.parametrize("data", VEC_2D_INPUT)
def test_vec2d(data: Vec2DLike) -> None:
    vec = Vec2D(data)
    assert_correct_vec2d(vec)


@pytest.mark.parametrize("data", VEC_3D_INPUT)
def test_vec3d(data: Vec3DLike) -> None:
    vec = Vec3D(data)
    assert_correct_vec3d(vec)


@pytest.mark.parametrize("data", VEC_4D_INPUT)
def test_vec4d(data: Vec4DLike) -> None:
    vec = Vec4D(data)
    assert_correct_vec4d(vec)


@pytest.mark.parametrize("data", vec2ds_arrays)
def test_vec2d_array_valid(data: Vec2DArrayLike) -> None:
    assert Vec2DBatch(data) == vec2ds_expected(data)


@pytest.mark.parametrize("data", vec3ds_arrays)
def test_vec3d_array_valid(data: Vec3DArrayLike) -> None:
    assert Vec3DBatch(data) == vec3ds_expected(data)


@pytest.mark.parametrize("data", vec4ds_arrays)
def test_vec4d_array_valid(data: Vec4DArrayLike) -> None:
    assert Vec4DBatch(data) == vec4ds_expected(data)


VEC_2D_INVALID_ARRAYS_INPUT = [
    [1],
    [1, 2, 3],
    [1, 2, 3, 4, 5],
    [[1], [2]],
    [[1, 2, 3], [4, 5]],
    [[1, 2, 3], [4, 5, 6]],
]


@pytest.mark.parametrize("data", VEC_2D_INVALID_ARRAYS_INPUT)
def test_vec2d_array_invalid(data: Vec2DArrayLike) -> None:
    with pytest.raises(ValueError):
        Vec2DBatch(data)
    with pytest.raises(ValueError):
        Vec2DBatch(np.array(data))


VEC_3D_INVALID_ARRAYS_INPUT = [
    [1],
    [1, 2],
    [1, 2, 3, 4],
    [1, 2, 3, 4, 5],
    [[1], [2], [3]],
    [[1, 2, 3], [4, 5]],
    [[1, 2, 3], [4, 5, 6, 7]],
    [[1, 2, 3, 4], [4, 5, 6]],
]


@pytest.mark.parametrize("data", VEC_3D_INVALID_ARRAYS_INPUT)
def test_vec3d_array_invalid(data: Vec3DArrayLike) -> None:
    with pytest.raises(ValueError):
        Vec3DBatch(data)
    with pytest.raises(ValueError):
        Vec3DBatch(np.array(data))


VEC_4D_INVALID_ARRAYS_INPUT = [
    [1],
    [1, 2],
    [1, 2, 3],
    [1, 2, 3, 4, 5],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [[1], [2], [3], [4]],
    [[1, 2, 3, 4], [4, 5]],
    [[1, 2, 3, 4], [4, 5, 6, 7, 8]],
    [[1, 2, 3, 4, 5], [4, 5, 6, 7]],
]


@pytest.mark.parametrize("data", VEC_4D_INVALID_ARRAYS_INPUT)
def test_vec4d_array_invalid(data: Vec4DArrayLike) -> None:
    with pytest.raises(ValueError):
        Vec4DBatch(data)
    with pytest.raises(ValueError):
        Vec4DBatch(np.array(data))
