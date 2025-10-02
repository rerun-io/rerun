from __future__ import annotations

import numpy as np
import pytest
import rerun as rr
from rerun.datatypes import Rgba32ArrayLike, Rgba32Batch
from rerun.error_utils import RerunWarning

CASES: list[tuple[Rgba32ArrayLike, Rgba32ArrayLike]] = [
    (
        [],
        [],
    ),
    (
        0x12345678,
        [0x12345678],
    ),
    (
        [0x12345678],
        [0x12345678],
    ),
    (
        [[0x12345678]],
        [0x12345678],
    ),
    (
        [1, 2, 3],
        [0x010203FF],
    ),
    (
        [[1, 2, 3]],
        [0x010203FF],
    ),
    (
        [1.0, 1.0, 1.0],
        [0xFFFFFFFF],
    ),
    (
        [[1.0, 1.0, 1.0]],
        [0xFFFFFFFF],
    ),
    (
        [1, 2, 3, 4],
        [0x01020304],
    ),
    (
        [[1, 2, 3, 4]],
        [0x01020304],
    ),
    (
        [0x11000000, 0x00220000],
        [0x11000000, 0x00220000],
    ),
    (
        [[1, 2, 3, 4], [5, 6, 7, 8]],
        [0x01020304, 0x05060708],
    ),
    (
        [1, 2, 3, 4, 5, 6, 7, 8],
        [1, 2, 3, 4, 5, 6, 7, 8],
    ),
    (
        np.array([1, 2, 3, 4, 5, 6, 7, 8], dtype=np.uint8),
        [0x01020304, 0x05060708],
    ),
    (
        [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]],
        [0x01020304, 0x05060708, 0x090A0B0C],
    ),
    (
        np.array([0x11000000, 0x00220000, 0x00003300], dtype=np.uint32),
        [0x11000000, 0x00220000, 0x00003300],
    ),
]


def test_rgba() -> None:
    for input, expected in CASES:
        data = Rgba32Batch(input)
        assert data.as_arrow_array().to_pylist() == expected


AMBIGUOUS_CASES: list[tuple[Rgba32ArrayLike, Rgba32ArrayLike]] = [
    (
        [0x11000000, 0x00220000, 0x00003300],
        [0x11000000, 0x00220000, 0x00003300],
    ),
    (
        [0x11000000, 0x00220000, 0x00003300, 0x00000044],
        [0x11000000, 0x00220000, 0x00003300, 0x00000044],
    ),
]


def test_ambiguous_rgba() -> None:
    rr.init("rerun_example_ambiguous_rgba", strict=False)
    for input, expected in AMBIGUOUS_CASES:
        with pytest.warns(RerunWarning) as warnings:
            data = Rgba32Batch(input)
            assert data.as_arrow_array().to_pylist() == expected
            assert len(warnings) == 1
