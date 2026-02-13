from __future__ import annotations

import itertools
from typing import Any, cast

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components import (
    LineStrip2DArrayLike,
    LineStrip2DBatch,
)
from rerun.datatypes import ClassIdArrayLike, Float32ArrayLike, Float32Like, Rgba32ArrayLike, Utf8ArrayLike, Vec2D

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    draw_order_expected,
    draw_orders,
    labels_arrays,
    labels_expected,
    none_empty_or_value,
    radii_arrays,
    radii_expected,
)

strips_arrays: list[LineStrip2DArrayLike] = [
    [],
    np.array([]),
    [
        [[0, 0], [2, 1], [4, -1], [6, 0]],
        [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
    ],
    [
        [Vec2D([0, 0]), (2, 1), [4, -1], (6, 0)],  # type: ignore[list-item]
        [Vec2D([0, 3]), (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],  # type: ignore[list-item]
    ],
    [
        np.array([[0, 0], (2, 1), [4, -1], (6, 0)], dtype=np.float32),
        np.array([[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]], dtype=np.float32),
    ],
    [
        torch.tensor([[0, 0], (2, 1), [4, -1], (6, 0)], dtype=torch.float32),
        torch.tensor([[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]], dtype=torch.float32),
    ],
    # NOTE: Not legal -- non-homogeneous.
    # np.array([
    #     [[0, 0], (2, 1), [4, -1], (6, 0)],
    #     [[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],
    # ]),
]


def line_strips2d_expected(obj: Any) -> Any:
    expected = none_empty_or_value(
        obj,
        [
            [[0, 0], [2, 1], [4, -1], [6, 0]],
            [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
        ],
    )

    return LineStrip2DBatch(expected)


def test_line_strips2d() -> None:
    all_arrays = itertools.zip_longest(
        strips_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        draw_orders,
        class_ids_arrays,
    )

    for strips, radii, colors, labels, draw_order, class_ids in all_arrays:
        strips = strips if strips is not None else strips_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        strips = cast("LineStrip2DArrayLike", strips)
        radii = cast("Float32ArrayLike | None", radii)
        colors = cast("Rgba32ArrayLike | None", colors)
        labels = cast("Utf8ArrayLike | None", labels)
        draw_order = cast("Float32Like | None", draw_order)
        class_ids = cast("ClassIdArrayLike | None", class_ids)

        print(
            f"rr.LineStrips2D(\n"
            f"    {strips}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    draw_order={draw_order!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")",
        )
        arch = rr.LineStrips2D(
            strips,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
        )
        print(f"{arch}\n")

        assert arch.strips == line_strips2d_expected(strips)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.draw_order == draw_order_expected(draw_order)
        assert arch.class_ids == class_ids_expected(class_ids)


@pytest.mark.parametrize(
    "data",
    [
        [[[0, 0], [2, 1]], [[4, -1], [6, 0]]],
        np.array([[0, 0], [2, 1], [4, -1], [6, 0]]).reshape([2, 2, 2]),
    ],
)
def test_line_segments2d(data: LineStrip2DArrayLike) -> None:
    arch = rr.LineStrips2D(data)

    assert arch.strips == LineStrip2DBatch([
        [[0, 0], [2, 1]],
        [[4, -1], [6, 0]],
    ])


def test_single_line_strip2d() -> None:
    # Regression test for #3643
    # Single linestrip can be passed and is not interpreted as batch of zero sized line strips.
    reference = rr.LineStrips2D([rr.components.LineStrip2D([[0, 0], [1, 1]])])
    assert reference.strips is not None and len(reference.strips) == 1
    assert reference == rr.LineStrips2D(rr.components.LineStrip2D([[0, 0], [1, 1]]))
    assert reference == rr.LineStrips2D([[[0, 0], [1, 1]]])
    assert reference == rr.LineStrips2D([[0, 0], [1, 1]])
    assert reference == rr.LineStrips2D(np.array([[0, 0], [1, 1]]))
    assert reference == rr.LineStrips2D([np.array([0, 0]), np.array([1, 1])])


def test_line_strip2d_invalid_shapes() -> None:
    rr.set_strict_mode(True)

    # We used to support flat arrays but this becomes too ambiguous when passing a single strip.
    with pytest.raises(ValueError):
        rr.LineStrips2D(
            [
                [0, 0, 2, 1, 4, -1, 6, 0],
                [0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3],
            ],
        )
    with pytest.raises(ValueError):
        rr.LineStrips2D(
            [
                np.array([0, 0, 2, 1, 4, -1, 6, 0], dtype=np.float32),
                np.array([0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3], dtype=np.float32),
            ],
        )

    # not homogeneous numpy arrays
    with pytest.raises(ValueError):
        rr.LineStrips2D(
            np.array([
                [[0, 0], (2, 1), [4, -1], (6, 0)],
                [[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],
            ]),
        )
    with pytest.raises(ValueError):
        rr.LineStrips2D(
            np.array([
                [0, 0, 2, 1, 4, -1, 6, 0],
                [0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3],
            ]),
        )


if __name__ == "__main__":
    test_line_strips2d()
