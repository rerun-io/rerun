from __future__ import annotations

import itertools
from typing import Any, Optional, cast

import numpy as np
import pytest
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    draw_order_expected,
    draw_orders,
    instance_keys_arrays,
    instance_keys_expected,
    labels_arrays,
    labels_expected,
    none_empty_or_value,
    radii_arrays,
    radii_expected,
)

strips_arrays: list[rrc.LineStrip2DArrayLike] = [
    [],
    np.array([]),
    [
        [rrd.Vec2D([0, 0]), (2, 1), [4, -1], (6, 0)],
        [rrd.Vec2D([0, 3]), (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]],
    ],
    [
        [0, 0, 2, 1, 4, -1, 6, 0],
        [0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3],
    ],
    [
        np.array([[0, 0], (2, 1), [4, -1], (6, 0)], dtype=np.float32),
        np.array([[0, 3], (1, 4), [2, 2], (3, 4), [4, 2], (5, 4), [6, 3]], dtype=np.float32),
    ],
    [
        np.array([0, 0, 2, 1, 4, -1, 6, 0], dtype=np.float32),
        np.array([0, 3, 1, 4, 2, 2, 3, 4, 4, 2, 5, 4, 6, 3], dtype=np.float32),
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

    return rrc.LineStrip2DArray.from_similar(expected)


def test_line_strips2d() -> None:
    all_arrays = itertools.zip_longest(
        strips_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        draw_orders,
        class_ids_arrays,
        instance_keys_arrays,
    )

    for strips, radii, colors, labels, draw_order, class_ids, instance_keys in all_arrays:
        strips = strips if strips is not None else strips_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        strips = cast(rrc.LineStrip2DArrayLike, strips)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrd.ColorArrayLike], colors)
        labels = cast(Optional[rrd.Utf8ArrayLike], labels)
        draw_order = cast(Optional[rrc.DrawOrderArrayLike], draw_order)
        class_ids = cast(Optional[rrd.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.LineStrips2D(\n"
            f"    {strips}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    draw_order={draw_order}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.LineStrips2D(
            strips,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.strips == line_strips2d_expected(strips)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.draw_order == draw_order_expected(draw_order)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.instance_keys == instance_keys_expected(instance_keys)


@pytest.mark.parametrize(
    "data",
    [
        [[[0, 0], [2, 1]], [[4, -1], [6, 0]]],
        np.array([[0, 0], [2, 1], [4, -1], [6, 0]]).reshape([2, 2, 2]),
    ],
)
def test_line_segments2d(data: rrc.LineStrip2DArrayLike) -> None:
    arch = rr2.LineStrips2D(data)

    assert arch.strips == rrc.LineStrip2DArray.from_similar(
        [
            [[0, 0], [2, 1]],
            [[4, -1], [6, 0]],
        ]
    )


if __name__ == "__main__":
    test_line_strips2d()
