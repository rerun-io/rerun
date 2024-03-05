from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import pytest
import rerun as rr
from rerun.components import (
    Color,
    ColorBatch,
    Position3DBatch,
    RadiusArrayLike,
)
from rerun.datatypes import ClassIdArrayLike, KeypointIdArrayLike, Rgba32ArrayLike, Utf8ArrayLike, Vec3DArrayLike

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    keypoint_ids_arrays,
    keypoint_ids_expected,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
)
from .common_arrays import (
    vec3ds_arrays as positions_arrays,
)
from .common_arrays import (
    vec3ds_expected as positions_expected,
)


def test_points3d() -> None:
    all_arrays = itertools.zip_longest(
        positions_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_ids_arrays,
        keypoint_ids_arrays,
    )

    for positions, radii, colors, labels, class_ids, keypoint_ids in all_arrays:
        positions = positions if positions is not None else positions_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        positions = cast(Vec3DArrayLike, positions)
        radii = cast(Optional[RadiusArrayLike], radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)
        keypoint_ids = cast(Optional[KeypointIdArrayLike], keypoint_ids)

        print(
            f"rr.Points3D(\n"
            f"    {positions}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    class_ids={class_ids!r}\n"
            f"    keypoint_ids={keypoint_ids!r}\n"
            f")"
        )
        arch = rr.Points3D(
            positions,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
        )
        print(f"{arch}\n")

        assert arch.positions == positions_expected(positions, Position3DBatch)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.keypoint_ids == keypoint_ids_expected(keypoint_ids)


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
def test_point3d_single_color(data: Rgba32ArrayLike) -> None:
    pts = rr.Points3D(positions=np.zeros((5, 3)), colors=data)

    assert pts.colors == ColorBatch(Color([0, 128, 0, 255]))


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
def test_point3d_multiple_colors(data: Rgba32ArrayLike) -> None:
    pts = rr.Points3D(positions=np.zeros((5, 3)), colors=data)

    assert pts.colors == ColorBatch(
        [
            Color([0, 128, 0, 255]),
            Color([128, 0, 0, 255]),
        ]
    )


if __name__ == "__main__":
    test_points3d()
