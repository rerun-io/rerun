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
    instance_keys_arrays,
    instance_keys_expected,
    is_empty,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
)

# fmt: off
strips_arrays: list[rrc.LineStrip3DArrayLike] = [
    [],
    [
        [rrd.Vec3D([0, 0, 2]), (1, 0, 2), [1, 1, 2], (0, 1, 2)],  # type: ignore[list-item]
        [rrd.Vec3D([0, 0, 0]), (0, 0, 1), [1, 0, 0], (1, 0, 1),
                   [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)]],  # type: ignore[list-item]
    [
        [0, 0, 2, 1, 0, 2, 1, 1, 2, 0, 1, 2],
        [0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1],
    ],
    [
        np.array([([0, 0, 2]), (1, 0, 2), [1, 1, 2], (0, 1, 2)], dtype=np.float32),
        np.array([([0, 0, 0]), (0, 0, 1), [1, 0, 0], (1, 0, 1), [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)], dtype=np.float32), # noqa
    ],
    [
        np.array([0, 0, 2, 1, 0, 2, 1, 1, 2, 0, 1, 2], dtype=np.float32),
        np.array([0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, ], dtype=np.float32),
    ],
    # NOTE: Not legal -- non-homogeneous.
    # np.array([
    #     [([0, 0, 2]), [1, 0, 2], [1, 1, 2], [0, 1, 2]],
    #     [([0, 0, 0]), [0, 0, 1], [1, 0, 0], [1, 0, 1], [1, 1, 0], [1, 1, 1], [0, 1, 0], [0, 1, 1]],
    # ]),
]
# fmt: on


def line_strips3d_expected(empty: bool) -> Any:
    return rrc.LineStrip3DArray.from_similar(
        []
        if empty
        else [
            [[0, 0, 2], [1, 0, 2], [1, 1, 2], [0, 1, 2]],
            [[0, 0, 0], [0, 0, 1], [1, 0, 0], [1, 0, 1], [1, 1, 0], [1, 1, 1], [0, 1, 0], [0, 1, 1]],
        ]
    )


def test_line_strips3d() -> None:
    all_arrays = itertools.zip_longest(
        strips_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_ids_arrays,
        instance_keys_arrays,
    )

    for strips, radii, colors, labels, class_ids, instance_keys in all_arrays:
        strips = strips if strips is not None else strips_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        strips = cast(rrc.LineStrip3DArrayLike, strips)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrc.ColorArrayLike], colors)
        labels = cast(Optional[rrc.LabelArrayLike], labels)
        class_ids = cast(Optional[rrc.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.LineStrips3D(\n"
            f"    {strips}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.LineStrips3D(
            strips,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.strips == line_strips3d_expected(is_empty(strips))
        assert arch.radii == radii_expected(is_empty(radii))
        assert arch.colors == colors_expected(is_empty(colors))
        assert arch.labels == labels_expected(is_empty(labels))
        assert arch.class_ids == class_ids_expected(is_empty(class_ids))
        assert arch.instance_keys == instance_keys_expected(is_empty(instance_keys))


@pytest.mark.parametrize(
    "data",
    [
        [[[0, 0, 0], [0, 0, 1]], [[1, 0, 0], [1, 0, 1]], [[1, 1, 0], [1, 1, 1]], [[0, 1, 0], [0, 1, 1]]],
        np.array(
            [
                [0, 0, 0],
                [0, 0, 1],
                [1, 0, 0],
                [1, 0, 1],
                [1, 1, 0],
                [1, 1, 1],
                [0, 1, 0],
                [0, 1, 1],
            ],
        ).reshape([4, 2, 3]),
    ],
)
def test_line_segments3d(data: rrc.LineStrip3DArrayLike) -> None:
    arch = rr2.LineStrips3D(data)

    assert arch.strips == rrc.LineStrip3DArray.from_similar(
        [[[0, 0, 0], [0, 0, 1]], [[1, 0, 0], [1, 0, 1]], [[1, 1, 0], [1, 1, 1]], [[0, 1, 0], [0, 1, 1]]],
    )


if __name__ == "__main__":
    test_line_strips3d()
