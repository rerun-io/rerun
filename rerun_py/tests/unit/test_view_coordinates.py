from __future__ import annotations

from typing import Any

import rerun.components as rrc
import rerun.experimental as rr2
from rerun.archetypes.view_coordinates import ViewCoordinates

from tests.unit.common_arrays import none_empty_or_value


def view_coordinates_expected(obj: Any) -> rrc.ViewCoordinatesArray:
    expected = none_empty_or_value(
        obj, [rrc.ViewCoordinates.RIGHT, rrc.ViewCoordinates.DOWN, rrc.ViewCoordinates.FORWARD]
    )

    return rrc.ViewCoordinatesArray.from_similar(expected)


VIEW_COORDINATES_INPUTS: list[rrc.ViewCoordinates] = [
    None,
    rrc.ViewCoordinates(
        [
            rrc.ViewCoordinates.RIGHT,
            rrc.ViewCoordinates.DOWN,
            rrc.ViewCoordinates.FORWARD,
        ]
    ),
    [
        rrc.ViewCoordinates.RIGHT,
        rrc.ViewCoordinates.DOWN,
        rrc.ViewCoordinates.FORWARD,
    ],
    # ViewCoordinates.RDF,
]


def test_view_coordinates() -> None:
    for coordinates in VIEW_COORDINATES_INPUTS:
        arch = ViewCoordinates(coordinates)

        print(f"rr2.ViewCoordinates(\n" f"    {coordinates}\n" f")")
        arch = rr2.ViewCoordinates(
            coordinates,
        )
        print(f"{arch}\n")

        assert arch.coordinates == view_coordinates_expected(coordinates)
