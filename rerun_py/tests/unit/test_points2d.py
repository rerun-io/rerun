from __future__ import annotations

import itertools
from typing import Optional, cast

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
    is_empty,
    keypoint_ids_arrays,
    keypoint_ids_expected,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
)
from .common_arrays import (
    vec2ds_arrays as points_arrays,
)
from .common_arrays import (
    vec2ds_expected as points_expected,
)


def test_points2d() -> None:
    all_arrays = itertools.zip_longest(
        points_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        draw_orders,
        class_ids_arrays,
        keypoint_ids_arrays,
        instance_keys_arrays,
    )

    for points, radii, colors, labels, draw_order, class_ids, keypoint_ids, instance_keys in all_arrays:
        points = points if points is not None else points_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        points = cast(rrd.Vec2DArrayLike, points)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrc.ColorArrayLike], colors)
        labels = cast(Optional[rrc.LabelArrayLike], labels)
        draw_order = cast(Optional[rrc.DrawOrderArrayLike], draw_order)
        class_ids = cast(Optional[rrc.ClassIdArrayLike], class_ids)
        keypoint_ids = cast(Optional[rrc.KeypointIdArrayLike], keypoint_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Points2D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    draw_order={draw_order}\n"
            f"    class_ids={class_ids}\n"
            f"    keypoint_ids={keypoint_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Points2D(
            points,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.points == points_expected(is_empty(points), rrc.Point2DArray)
        assert arch.radii == radii_expected(is_empty(radii))
        assert arch.colors == colors_expected(is_empty(colors))
        assert arch.labels == labels_expected(is_empty(labels))
        assert arch.draw_order == draw_order_expected(is_empty(draw_order))
        assert arch.class_ids == class_ids_expected(is_empty(class_ids))
        assert arch.keypoint_ids == keypoint_ids_expected(is_empty(keypoint_ids))
        assert arch.instance_keys == instance_keys_expected(is_empty(instance_keys))


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
def test_point2d_single_color(data: rrc.ColorArrayLike) -> None:
    pts = rr2.Points2D(points=np.zeros((5, 2)), colors=data)

    assert pts.colors == rrc.ColorArray.from_similar(rrc.Color([0, 128, 0, 255]))


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
def test_point2d_multiple_colors(data: rrc.ColorArrayLike) -> None:
    pts = rr2.Points2D(points=np.zeros((5, 2)), colors=data)

    assert pts.colors == rrc.ColorArray.from_similar(
        [
            rrc.Color([0, 128, 0, 255]),
            rrc.Color([128, 0, 0, 255]),
        ]
    )


if __name__ == "__main__":
    test_points2d()
