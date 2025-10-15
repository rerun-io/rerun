from __future__ import annotations

from typing import TYPE_CHECKING, Any

import rerun.components as rrc
from rerun.archetypes import ViewCoordinates

from .common_arrays import none_empty_or_value

if TYPE_CHECKING:
    from rerun.datatypes.view_coordinates import ViewCoordinatesArrayLike


def view_coordinates_expected(obj: Any) -> rrc.ViewCoordinatesBatch:
    expected = none_empty_or_value(
        obj,
        [rrc.ViewCoordinates.ViewDir.Right, rrc.ViewCoordinates.ViewDir.Down, rrc.ViewCoordinates.ViewDir.Forward],
    )

    return rrc.ViewCoordinatesBatch(expected)


VIEW_COORDINATES_INPUTS: list[ViewCoordinatesArrayLike] = [
    rrc.ViewCoordinates([
        rrc.ViewCoordinates.ViewDir.Right,
        rrc.ViewCoordinates.ViewDir.Down,
        rrc.ViewCoordinates.ViewDir.Forward,
    ]),
    [
        rrc.ViewCoordinates.ViewDir.Right,
        rrc.ViewCoordinates.ViewDir.Down,
        rrc.ViewCoordinates.ViewDir.Forward,
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
