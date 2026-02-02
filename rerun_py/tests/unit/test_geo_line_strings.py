from __future__ import annotations

import itertools
from typing import Any, cast

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components import (
    GeoLineStringArrayLike,
    GeoLineStringBatch,
)
from rerun.datatypes import DVec2D, Float32ArrayLike, Rgba32ArrayLike

from .common_arrays import (
    colors_arrays,
    colors_expected,
    none_empty_or_value,
    radii_arrays,
    radii_expected,
)

geo_line_strings_arrays: list[GeoLineStringArrayLike] = [
    [],
    np.array([]),
    [
        [[0, 0], [2, 1], [4, -1], [6, 0]],
        [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
    ],
    [
        [DVec2D([0, 0]), (2, 1), [4, -1], (6, 0)],  # type: ignore[list-item]
        [DVec2D([0, 3]), (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],  # type: ignore[list-item]
    ],
    [
        np.array([[0, 0], (2, 1), [4, -1], (6, 0)], dtype=np.float64),
        np.array([[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]], dtype=np.float64),
    ],
    [
        torch.tensor([[0, 0], (2, 1), [4, -1], (6, 0)], dtype=torch.float64),
        torch.tensor([[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]], dtype=torch.float64),
    ],
    # NOTE: Not legal -- non-homogeneous.
    # np.array([
    #     [[0, 0], (2, 1), [4, -1], (6, 0)],
    #     [[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],
    # ]),
]


def geo_line_strings_expected(obj: Any) -> Any:
    expected = none_empty_or_value(
        obj,
        [
            [[0, 0], [2, 1], [4, -1], [6, 0]],
            [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
        ],
    )

    return GeoLineStringBatch(expected)


def test_geo_line_strings() -> None:
    all_arrays = itertools.zip_longest(
        geo_line_strings_arrays,
        radii_arrays,
        colors_arrays,
    )

    for strips, radii, colors in all_arrays:
        strips = strips if strips is not None else geo_line_strings_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info through zip_longest
        strips = cast("GeoLineStringArrayLike", strips)
        radii = cast("Float32ArrayLike | None", radii)
        colors = cast("Rgba32ArrayLike | None", colors)

        print(f"rr.GeoLineStrings(\n    lat_lon={strips}\n    radii={radii!r}\n    colors={colors!r}\n)")
        arch = rr.GeoLineStrings(
            lat_lon=strips,
            radii=radii,
            colors=colors,
        )
        print(f"{arch}\n")

        assert arch.line_strings == geo_line_strings_expected(strips)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)


@pytest.mark.parametrize(
    "data",
    [
        [[[0, 0], [2, 1]], [[4, -1], [6, 0]]],
        np.array([[0, 0], [2, 1], [4, -1], [6, 0]]).reshape([2, 2, 2]),
    ],
)
def test_geo_line_strings_segment(data: GeoLineStringArrayLike) -> None:
    arch = rr.GeoLineStrings(lat_lon=data)

    assert arch.line_strings == GeoLineStringBatch([
        [[0, 0], [2, 1]],
        [[4, -1], [6, 0]],
    ])


def test_geo_line_strings_single_line() -> None:
    # Regression test for #3643
    # Single line string can be passed and is not interpreted as a batch of zero-sized line strings.
    reference = rr.GeoLineStrings(lat_lon=[rr.components.GeoLineString(lat_lon=[[0, 0], [1, 1]])])
    assert reference.line_strings is not None and len(reference.line_strings) == 1
    assert reference == rr.GeoLineStrings(lat_lon=rr.components.GeoLineString(lat_lon=[[0, 0], [1, 1]]))
    assert reference == rr.GeoLineStrings(lat_lon=[[[0, 0], [1, 1]]])
    assert reference == rr.GeoLineStrings(lat_lon=[[0, 0], [1, 1]])
    assert reference == rr.GeoLineStrings(lat_lon=np.array([[0, 0], [1, 1]]))
    assert reference == rr.GeoLineStrings(lat_lon=[np.array([0, 0]), np.array([1, 1])])


def test_geo_line_strings_invalid_shapes() -> None:
    rr.set_strict_mode(True)

    # We used to support flat arrays but this becomes too ambiguous when passing a single strip.
    with pytest.raises(ValueError):
        rr.GeoLineStrings(
            lat_lon=[
                [0, 0, 2, 1, 4, -1, 6, 0],
                [0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3],
            ],
        )
    with pytest.raises(ValueError):
        rr.GeoLineStrings(
            lat_lon=[
                np.array([0, 0, 2, 1, 4, -1, 6, 0], dtype=np.float64),
                np.array([0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3], dtype=np.float64),
            ],
        )

    # not homogeneous numpy arrays
    with pytest.raises(ValueError):
        rr.GeoLineStrings(
            lat_lon=np.array([
                [[0, 0], (2, 1), [4, -1], (6, 0)],
                [[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],
            ]),
        )
    with pytest.raises(ValueError):
        rr.GeoLineStrings(
            lat_lon=np.array([
                [0, 0, 2, 1, 4, -1, 6, 0],
                [0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3],
            ]),
        )


if __name__ == "__main__":
    test_geo_line_strings()
