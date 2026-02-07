from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint import components as blueprint_components

from .common_arrays import none_empty_or_value

if TYPE_CHECKING:
    from rerun.datatypes.bool import BoolLike


def test_scalar_axis() -> None:
    rr.set_strict_mode(True)

    corners = [
        rrb.Corner2D.LeftTop,
        "lefttop",
        None,
    ]
    visible_array = [
        None,
        True,
    ]

    all_arrays = itertools.zip_longest(
        corners,
        visible_array,
    )

    for corner, visible in all_arrays:
        corner = cast("blueprint_components.Corner2DLike | None", corner)
        visible = cast("BoolLike | None", visible)

        print(
            f"rr.PlotLegend(\n    corner={corner!r}\n    visible={visible!r}\n)",
        )
        arch = rrb.PlotLegend(
            corner=corner,
            visible=visible,
        )
        print(f"{arch}\n")

        assert arch.corner == blueprint_components.Corner2DBatch._converter(
            none_empty_or_value(corner, rrb.Corner2D.LeftTop),
        )
        assert arch.visible == rr.components.VisibleBatch._converter(none_empty_or_value(visible, [True]))
