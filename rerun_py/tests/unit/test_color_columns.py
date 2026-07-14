"""
Tests for color interpretation in the `.columns()` path.

Which infers partition sizes from the input shape and the arrow array length.
Colors are tricky because inputs like `(N, 3)` arrays of RGB values get packed into `N`
uint32 values, so the arrow array length doesn't match the trailing input dimensions.
"""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
import pytest
import rerun as rr


def get_color_column(columns: list[Any]) -> list[Any]:
    """Extract the color column from a columns() result and return its pylist."""
    for col in columns:
        if "color" in col.component_descriptor().component.lower():
            return col.as_arrow_array().to_pylist()  # type: ignore[no-any-return]
    raise ValueError("No color column found")


class TestSingleColorPerRow:
    """Each row has exactly one color."""

    def test_rgb_list_per_row(self) -> None:
        colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255]]
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((3, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 3
        assert all(len(row) == 1 for row in result)

    def test_rgba_list_per_row(self) -> None:
        colors = [[255, 0, 0, 128], [0, 255, 0, 128]]
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 2
        assert all(len(row) == 1 for row in result)

    def test_rgb_numpy_per_row(self) -> None:
        colors = np.array([[255, 0, 0], [0, 255, 0], [0, 0, 255]], dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((3, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 3
        assert all(len(row) == 1 for row in result)

    def test_rgba_numpy_per_row(self) -> None:
        colors = np.array([[255, 0, 0, 255], [0, 255, 0, 255]], dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 2
        assert all(len(row) == 1 for row in result)

    def test_packed_uint32(self) -> None:
        colors = np.array([0xFF0000FF, 0x00FF00FF], dtype=np.uint32)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 2
        assert result == [[0xFF0000FF], [0x00FF00FF]]

    def test_float_rgb_per_row(self) -> None:
        colors = np.array([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 2
        assert all(len(row) == 1 for row in result)

    def test_float_rgba_per_row(self) -> None:
        colors = np.array([[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5]], dtype=np.float32)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 2
        assert all(len(row) == 1 for row in result)


class TestColorValues:
    """Verify the actual packed values are correct."""

    def test_rgb_packs_with_full_alpha(self) -> None:
        colors = [[255, 0, 0]]
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert result == [[0xFF0000FF]]

    def test_rgba_preserves_alpha(self) -> None:
        colors = [[255, 0, 0, 128]]
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert result == [[0xFF000080]]

    def test_uint8_rgb(self) -> None:
        colors = np.array([[1, 2, 3]], dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert result == [[0x010203FF]]

    def test_uint8_rgba(self) -> None:
        colors = np.array([[1, 2, 3, 4]], dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert result == [[0x01020304]]

    def test_float_rgb_white(self) -> None:
        colors = np.array([[1.0, 1.0, 1.0]], dtype=np.float32)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert result == [[0xFFFFFFFF]]

    def test_float_rgba_half_alpha(self) -> None:
        colors = np.array([[1.0, 1.0, 1.0, 0.5]], dtype=np.float32)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        packed = result[0][0]
        # RGB should be 0xFF each, alpha should be ~128
        assert (packed >> 24) & 0xFF == 0xFF  # R
        assert (packed >> 16) & 0xFF == 0xFF  # G
        assert (packed >> 8) & 0xFF == 0xFF  # B
        assert (packed & 0xFF) == 128  # A


class TestConsistency:
    """The columnar path should produce the same packed values as the regular path."""

    @pytest.mark.parametrize(
        "colors",
        [
            [[255, 0, 0], [0, 255, 0]],
            [[255, 0, 0, 128], [0, 255, 0, 128]],
            np.array([[255, 0, 0], [0, 255, 0]], dtype=np.uint8),
            np.array([[255, 0, 0, 255], [0, 255, 0, 255]], dtype=np.uint8),
            np.array([0xFF0000FF, 0x00FF00FF], dtype=np.uint32),
            np.array([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32),
        ],
    )
    def test_columnar_matches_regular(self, colors: list[list[int]] | npt.NDArray[Any]) -> None:
        """Values from .columns() should match values from regular archetype construction."""
        regular = rr.GeoPoints(lat_lon=np.zeros((2, 2)), colors=colors)
        assert regular.colors is not None
        regular_values = regular.colors.as_arrow_array().to_pylist()

        columnar_result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((2, 2)),
                colors=colors,
            )
        ])
        columnar_values = [v for row in columnar_result for v in row]

        assert regular_values == columnar_values


class TestEdgeCases:
    """Edge cases for color handling."""

    def test_single_row(self) -> None:
        colors = np.array([[128, 64, 32]], dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((1, 2)),
                colors=colors,
            )
        ])
        assert len(result) == 1
        assert len(result[0]) == 1

    def test_many_rows(self) -> None:
        n = 1000
        colors = np.random.randint(0, 256, (n, 3), dtype=np.uint8)
        result = get_color_column([
            *rr.GeoPoints.columns(
                positions=np.zeros((n, 2)),
                colors=colors,
            )
        ])
        assert len(result) == n
        assert all(len(row) == 1 for row in result)
