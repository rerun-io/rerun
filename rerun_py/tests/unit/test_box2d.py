from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

import numpy as np
import numpy.typing as npt
import pytest
import rerun as rr
import torch
from rerun.components import (
    HalfSize2DBatch,
    Position2DBatch,
)

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    draw_order_expected,
    draw_orders,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
    vec2ds_arrays as centers_arrays,
    vec2ds_arrays as half_sizes_arrays,
    vec2ds_expected as centers_expected,
    vec2ds_expected as half_sizes_expected,
)

if TYPE_CHECKING:
    from rerun.datatypes import ClassIdArrayLike, Float32ArrayLike, Rgba32ArrayLike, Utf8ArrayLike, Vec2DArrayLike


def test_boxes2d() -> None:
    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        centers_arrays,
        colors_arrays,
        radii_arrays,
        labels_arrays,
        draw_orders,
        class_ids_arrays,
    )

    for half_sizes, centers, colors, radii, labels, draw_order, class_ids in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast("Vec2DArrayLike", half_sizes)
        centers = cast("Vec2DArrayLike", centers)
        radii = cast("Float32ArrayLike | None", radii)
        colors = cast("Rgba32ArrayLike | None", colors)
        labels = cast("Utf8ArrayLike | None", labels)
        draw_order = cast("Float32ArrayLike | None", draw_order)
        class_ids = cast("ClassIdArrayLike | None", class_ids)

        print(
            f"rr.Boxes2D(\n"
            f"    half_sizes={half_sizes}\n"
            f"    centers={centers}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    draw_order={draw_order!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")",
        )
        arch = rr.Boxes2D(
            half_sizes=half_sizes,
            centers=centers,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
        )
        print(f"{arch}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSize2DBatch)
        assert arch.centers == centers_expected(centers, Position2DBatch)
        assert arch.colors == colors_expected(colors)
        assert arch.radii == radii_expected(radii)
        assert arch.labels == labels_expected(labels)
        assert arch.draw_order == draw_order_expected(draw_order)
        assert arch.class_ids == class_ids_expected(class_ids)


def test_with_sizes() -> None:
    assert rr.Boxes2D(sizes=[1, 2]) == rr.Boxes2D(half_sizes=[0.5, 1])


def test_with_centers_and_sizes() -> None:
    assert rr.Boxes2D(centers=[1, 2], sizes=[4, 6]) == rr.Boxes2D(centers=[1, 2], half_sizes=[2, 3])


def test_with_mins_and_sizes() -> None:
    assert rr.Boxes2D(mins=[-1, -1], sizes=[2, 4]) == rr.Boxes2D(centers=[0, 1], half_sizes=[1, 2])


def test_with_array_xywh() -> None:
    assert rr.Boxes2D(mins=[1, 2], sizes=[3, 4]) == rr.Boxes2D(array=[1, 2, 3, 4], array_format=rr.Box2DFormat.XYWH)


def test_with_array_yxhw() -> None:
    assert rr.Boxes2D(mins=[1, 2], sizes=[3, 4]) == rr.Boxes2D(array=[2, 1, 4, 3], array_format=rr.Box2DFormat.YXHW)


def test_with_array_xyxy() -> None:
    assert rr.Boxes2D(mins=[1, 2], sizes=[2, 2]) == rr.Boxes2D(array=[1, 2, 3, 4], array_format=rr.Box2DFormat.XYXY)


def test_with_array_yxyx() -> None:
    assert rr.Boxes2D(mins=[1, 2], sizes=[2, 2]) == rr.Boxes2D(array=[2, 1, 4, 3], array_format=rr.Box2DFormat.YXYX)


def test_with_array_xcycwh() -> None:
    assert rr.Boxes2D(mins=[1, 1], sizes=[2, 4]) == rr.Boxes2D(array=[2, 3, 2, 4], array_format=rr.Box2DFormat.XCYCWH)


def test_with_array_xcycw2h2() -> None:
    assert rr.Boxes2D(mins=[1, 1], sizes=[2, 4]) == rr.Boxes2D(array=[2, 3, 1, 2], array_format=rr.Box2DFormat.XCYCW2H2)


@pytest.mark.parametrize(
    "array",
    [
        [1, 2, 3, 4],
        [1, 2, 3, 4],
        np.array([1, 2, 3, 4], dtype=np.float32),
        torch.asarray([1, 2, 3, 4], dtype=torch.float32),
    ],
)
def test_with_array_types(array: npt.ArrayLike) -> None:
    assert rr.Boxes2D(mins=[1, 2], sizes=[3, 4]) == rr.Boxes2D(array=array, array_format=rr.Box2DFormat.XYWH)


def test_invalid_parameter_combinations() -> None:
    rr.set_strict_mode(True)

    # invalid size/position combinations
    with pytest.raises(ValueError):
        rr.Boxes2D(half_sizes=[1, 2], sizes=[3, 4])
    with pytest.raises(ValueError):
        rr.Boxes2D(centers=[1, 2], mins=[3, 4])
    with pytest.raises(ValueError):
        rr.Boxes2D(mins=[3, 4])

    # with array
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6])
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6], array_format=rr.Box2DFormat.XYWH, half_sizes=[1, 2])
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6], array_format=rr.Box2DFormat.XYWH, sizes=[1, 2])
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6], array_format=rr.Box2DFormat.XYWH, mins=[1, 2])
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6], array_format=rr.Box2DFormat.XYWH, centers=[1, 2])
    with pytest.raises(ValueError):
        rr.Boxes2D(array=[3, 4, 5, 6], array_format="bonkers")  # type: ignore[arg-type]


if __name__ == "__main__":
    test_boxes2d()
    test_with_sizes()
    test_with_centers_and_sizes()
    test_with_mins_and_sizes()
