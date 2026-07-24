from __future__ import annotations

from typing import TYPE_CHECKING, Any

import rerun.components as rrc
import rerun.datatypes as rrd
from rerun.archetypes import ViewCoordinates

from .common_arrays import none_empty_or_value

if TYPE_CHECKING:
    from rerun.datatypes.view_coordinates import ViewCoordinatesArrayLike


def view_coordinates_expected(obj: Any) -> rrc.ViewCoordinatesBatch:
    expected = none_empty_or_value(
        obj,
        [rrd.ViewDir.Right, rrd.ViewDir.Down, rrd.ViewDir.Forward],
    )

    return rrc.ViewCoordinatesBatch(expected)


assert rrc.ViewCoordinates.ViewDir is rrd.ViewDir


VIEW_COORDINATES_INPUTS: list[ViewCoordinatesArrayLike] = [
    rrc.ViewCoordinates([
        rrd.ViewDir.Right,
        rrd.ViewDir.Down,
        rrd.ViewDir.Forward,
    ]),
    [
        rrd.ViewDir.Right,
        rrd.ViewDir.Down,
        rrd.ViewDir.Forward,
    ],
    rrc.ViewCoordinates.RDF,
    [rrc.ViewCoordinates.RDF],
]


def test_view_coordinates() -> None:
    for coordinates in VIEW_COORDINATES_INPUTS:
        # TODO(jleibs): Figure out why mypy is confused by this arg-type
        arch = ViewCoordinates(coordinates)  # type: ignore[arg-type]

        print(f"rr.ViewCoordinates(\n    {coordinates!s}\n)")
        print(f"{arch}\n")

        assert arch.xyz == view_coordinates_expected(coordinates)
