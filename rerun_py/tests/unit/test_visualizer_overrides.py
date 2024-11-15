from __future__ import annotations

from typing import Any

import rerun.blueprint.components as rrbc
from rerun.blueprint.datatypes.utf8list import Utf8ListArrayLike

from .common_arrays import none_empty_or_value


def visualizer_overrides_expected(obj: Any) -> rrbc.VisualizerOverridesBatch:
    expected = none_empty_or_value(obj, ["boxes3d"])

    return rrbc.VisualizerOverridesBatch(expected)


VISUALIZER_OVERRIDES_INPUT: list[Utf8ListArrayLike | None] = [
    None,
    [],
    "boxes3d",
    ["boxes3d"],
    rrbc.VisualizerOverrides("boxes3d"),
    rrbc.VisualizerOverrides(["boxes3d"]),
]


def test_view_coordinates() -> None:
    for input in VISUALIZER_OVERRIDES_INPUT:
        batch = rrbc.VisualizerOverridesBatch(input)  # type: ignore[arg-type]

        assert batch.as_arrow_array() == visualizer_overrides_expected(batch).as_arrow_array()
