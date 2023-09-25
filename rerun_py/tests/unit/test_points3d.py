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
    instance_keys_arrays,
    instance_keys_expected,
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
        instance_keys_arrays,
    )

    for positions, radii, colors, labels, class_ids, keypoint_ids, instance_keys in all_arrays:
        positions = positions if positions is not None else positions_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        positions = cast(rrd.Vec3DArrayLike, positions)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrd.ColorArrayLike], colors)
        labels = cast(Optional[rrd.Utf8ArrayLike], labels)
        class_ids = cast(Optional[rrd.ClassIdArrayLike], class_ids)
        keypoint_ids = cast(Optional[rrd.KeypointIdArrayLike], keypoint_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Points3D(\n"
            f"    {positions}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    keypoint_ids={keypoint_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Points3D(
            positions,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.positions == positions_expected(positions, rrc.Position3DBatch)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.keypoint_ids == keypoint_ids_expected(keypoint_ids)
        assert arch.instance_keys == instance_keys_expected(instance_keys)


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
def test_point3d_single_color(data: rrd.ColorArrayLike) -> None:
    pts = rr2.Points3D(positions=np.zeros((5, 3)), colors=data)

    assert pts.colors == rrc.ColorBatch(rrd.Color([0, 128, 0, 255]))


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
def test_point3d_multiple_colors(data: rrd.ColorArrayLike) -> None:
    pts = rr2.Points3D(positions=np.zeros((5, 3)), colors=data)

    assert pts.colors == rrc.ColorBatch(
        [
            rrd.Color([0, 128, 0, 255]),
            rrd.Color([128, 0, 0, 255]),
        ]
    )


if __name__ == "__main__":
    test_points3d()
