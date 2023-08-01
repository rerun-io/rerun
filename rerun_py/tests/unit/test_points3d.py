from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import pytest
import rerun.experimental as rr2
from rerun.experimental import cmp as rr_cmp
from rerun.experimental import dt as rr_dt

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
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


def test_points3d() -> None:
    points_arrays: list[rr_dt.Point3DArrayLike] = [
        [],
        np.array([]),
        # Point3DArrayLike: Sequence[Point3DLike]: Point3D
        [
            rr_dt.Point3D([1, 2, 3]),
            rr_dt.Point3D([4, 5, 6]),
        ],
        # Point3DArrayLike: Sequence[Point3DLike]: npt.NDArray[np.float32]
        [
            np.array([1, 2, 3], dtype=np.float32),
            np.array([4, 5, 6], dtype=np.float32),
        ],
        # Point3DArrayLike: Sequence[Point3DLike]: Tuple[float, float]
        [(1, 2, 3), (4, 5, 6)],
        # Point3DArrayLike: Sequence[Point3DLike]: Sequence[float]
        [1, 2, 3, 4, 5, 6],
        # Point3DArrayLike: npt.NDArray[np.float32]
        np.array([[1, 2, 3], [4, 5, 6]], dtype=np.float32),
        # Point3DArrayLike: npt.NDArray[np.float32]
        np.array([1, 2, 3, 4, 5, 6], dtype=np.float32),
    ]

    all_arrays = itertools.zip_longest(
        points_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_ids_arrays,
        keypoint_ids_arrays,
        instance_keys_arrays,
    )

    for points, radii, colors, labels, class_ids, keypoint_ids, instance_keys in all_arrays:
        points = points if points is not None else points_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        points = cast(Optional[rr_dt.Point3DArrayLike], points)
        radii = cast(Optional[rr_cmp.RadiusArrayLike], radii)
        colors = cast(Optional[rr_cmp.ColorArrayLike], colors)
        labels = cast(Optional[rr_cmp.LabelArrayLike], labels)
        class_ids = cast(Optional[rr_cmp.ClassIdArrayLike], class_ids)
        keypoint_ids = cast(Optional[rr_cmp.KeypointIdArrayLike], keypoint_ids)
        instance_keys = cast(Optional[rr_cmp.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Points3D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    keypoint_ids={keypoint_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Points3D(
            points,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.points == rr_cmp.Point3DArray.from_similar(
            [] if is_empty(points) else [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
        )
        assert arch.radii == radii_expected(is_empty(radii))
        assert arch.colors == colors_expected(is_empty(colors))
        assert arch.labels == labels_expected(is_empty(labels))
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
def test_point3d_single_color(data: rr_cmp.ColorArrayLike) -> None:
    pts = rr2.Points3D(points=np.zeros((5, 3)), colors=data)

    assert pts.colors == rr_cmp.ColorArray.from_similar(rr_cmp.Color([0, 128, 0, 255]))


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
def test_point3d_multiple_colors(data: rr_cmp.ColorArrayLike) -> None:
    pts = rr2.Points3D(points=np.zeros((5, 3)), colors=data)

    assert pts.colors == rr_cmp.ColorArray.from_similar(
        [
            rr_cmp.Color([0, 128, 0, 255]),
            rr_cmp.Color([128, 0, 0, 255]),
        ]
    )


if __name__ == "__main__":
    test_points3d()
