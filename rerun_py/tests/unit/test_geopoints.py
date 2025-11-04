from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

import numpy as np
import pytest
import rerun as rr
from rerun.components import (
    Color,
    ColorBatch,
    LatLonBatch,
)

from .common_arrays import (
    colors_arrays,
    colors_expected,
    dvec2ds_arrays as positions_arrays,
    dvec2ds_expected as positions_expected,
    radii_arrays,
    radii_expected,
)

if TYPE_CHECKING:
    from rerun.datatypes import (
        DVec2DArrayLike,
        Float32ArrayLike,
        Rgba32ArrayLike,
    )


def test_geopoints() -> None:
    all_arrays = itertools.zip_longest(
        positions_arrays,
        radii_arrays,
        colors_arrays,
    )

    for positions, radii, colors in all_arrays:
        positions = positions if positions is not None else positions_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info through zip_longest
        positions = cast("DVec2DArrayLike", positions)
        radii = cast("Float32ArrayLike | None", radii)
        colors = cast("Rgba32ArrayLike | None", colors)

        print(f"rr.GeoPoints(\n    lat_lon={positions}\n    radii={radii!r}\n    colors={colors!r}\n)")
        arch = rr.GeoPoints(
            lat_lon=positions,
            radii=radii,
            colors=colors,
        )
        print(f"{arch}\n")

        assert arch.positions == positions_expected(positions, LatLonBatch)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)


@pytest.mark.parametrize(
    "data",
    [
        [0, 128, 0, 255],
        [0, 128, 0],
        np.array((0, 128, 0, 255)),
        [0.0, 0.5, 0.0, 1.0],
        np.array((0.0, 0.5, 0.0, 1.0)),
    ],
)
def test_geopoint_single_color(data: Rgba32ArrayLike) -> None:
    pts = rr.GeoPoints(lat_lon=np.zeros((5, 2)), colors=data)

    assert pts.colors == ColorBatch(Color([0, 128, 0, 255]))


@pytest.mark.parametrize(
    "data",
    [
        [[0, 128, 0, 255], [128, 0, 0, 255]],
        [[0, 128, 0], [128, 0, 0]],
        np.array([[0, 128, 0, 255], [128, 0, 0, 255]]),
        np.array([0, 128, 0, 255, 128, 0, 0, 255], dtype=np.uint8),
        np.array([8388863, 2147483903], dtype=np.uint32),
        np.array([[0, 128, 0], [128, 0, 0]]),
        [[0.0, 0.5, 0.0, 1.0], [0.5, 0.0, 0.0, 1.0]],
        [[0.0, 0.5, 0.0], [0.5, 0.0, 0.0]],
        np.array([[0.0, 0.5, 0.0, 1.0], [0.5, 0.0, 0.0, 1.0]]),
        np.array([[0.0, 0.5, 0.0], [0.5, 0.0, 0.0]]),
        np.array([0.0, 0.5, 0.0, 1.0, 0.5, 0.0, 0.0, 1.0]),
        # Note: Sequence[int] is interpreted as a single color when they are 3 or 4 long. For other lengths, they
        # are interpreted as list of packed uint32 colors. Note that this means one cannot pass an len=N*4 flat list of
        # color components.
        [8388863, 2147483903],
    ],
)
def test_point2d_multiple_colors(data: Rgba32ArrayLike) -> None:
    pts = rr.GeoPoints(lat_lon=np.zeros((5, 2)), colors=data)

    assert pts.colors == ColorBatch([
        Color([0, 128, 0, 255]),
        Color([128, 0, 0, 255]),
    ])


if __name__ == "__main__":
    test_geopoints()
