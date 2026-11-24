from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    import rerun as rr


def assert_quat_equivalent(actual: rr.components.RotationQuatBatch, expected: list[float]) -> None:
    """Quaternions `q` and `-q` are the same rotation."""
    actual_values = actual.pa_array.to_pylist()[0]
    dot = sum(a * b for a, b in zip(actual_values, expected, strict=False))
    if dot < 0.0:
        expected = [-value for value in expected]
    assert actual_values == pytest.approx(expected)


def assert_translation_expected(actual: rr.components.Translation3DBatch, expected: tuple[float, float, float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    assert actual_values == pytest.approx(expected)
