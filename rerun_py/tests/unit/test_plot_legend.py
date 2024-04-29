from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint import components as blueprint_components

from .common_arrays import none_empty_or_value


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
        corner = cast(Optional[blueprint_components.Corner2DLike], corner)
        visible = cast(Optional[blueprint_components.VisibleLike], visible)

        print(
            f"rr.PlotLegend(\n"
            f"    corner={corner!r}\n"  #
            f"    visible={visible!r}\n"
            f")"
        )
        arch = rrb.PlotLegend(
            corner=corner,
            visible=visible,
        )
        print(f"{arch}\n")

        assert arch.corner == blueprint_components.Corner2DBatch._optional(
            none_empty_or_value(corner, rrb.Corner2D.LeftTop)
        )
        assert arch.visible == blueprint_components.VisibleBatch._optional(none_empty_or_value(visible, True))
