from __future__ import annotations

from typing import Any

import rerun.components as rrc
import rerun.experimental as rr2
from rerun.archetypes.view_coordinates import ViewCoordinates

from .common_arrays import none_empty_or_value


def view_coordinates_expected(obj: Any) -> rrc.ViewCoordinatesArray:
    expected = none_empty_or_value(
        obj, [rrc.ViewCoordinates.ViewDir.Right, rrc.ViewCoordinates.ViewDir.Down, rrc.ViewCoordinates.ViewDir.Forward]
    )

    return rrc.ViewCoordinatesArray.from_similar(expected)


VIEW_COORDINATES_INPUTS: list[rrc.ViewCoordinatesLike | None] = [
    None,
    rrc.ViewCoordinates(
        [
            rrc.ViewCoordinates.ViewDir.Right,
            rrc.ViewCoordinates.ViewDir.Down,
            rrc.ViewCoordinates.ViewDir.Forward,
        ]
    ),
    [
        rrc.ViewCoordinates.ViewDir.Right,
        rrc.ViewCoordinates.ViewDir.Down,
        rrc.ViewCoordinates.ViewDir.Forward,
    ],
    ViewCoordinates.RDF,
    [ViewCoordinates.RDF],
]


def test_view_coordinates() -> None:
    for coordinates in VIEW_COORDINATES_INPUTS:
        arch = ViewCoordinates(coordinates)

        print(f"rr2.ViewCoordinates(\n    {str(coordinates)}\n)")
        arch = rr2.ViewCoordinates(
            coordinates,
        )
        print(f"{arch}\n")

        assert arch.xyz == view_coordinates_expected(coordinates)
