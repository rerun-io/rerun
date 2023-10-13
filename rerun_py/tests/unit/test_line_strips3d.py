from __future__ import annotations

import itertools
from typing import Any, Optional, cast

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.components.instance_key import InstanceKeyArrayLike
from rerun.components.line_strip3d import LineStrip3DArrayLike, LineStrip3DBatch
from rerun.components.radius import RadiusArrayLike
from rerun.datatypes import Vec3D
from rerun.datatypes.class_id import ClassIdArrayLike
from rerun.datatypes.rgba32 import Rgba32ArrayLike
from rerun.datatypes.utf8 import Utf8ArrayLike

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    instance_keys_arrays,
    instance_keys_expected,
    labels_arrays,
    labels_expected,
    none_empty_or_value,
    radii_arrays,
    radii_expected,
)

# fmt: off
strips_arrays: list[LineStrip3DArrayLike] = [
    [],
    np.array([]),
    [
        [[0, 0, 2], [1, 0, 2], [1, 1, 2], (0, 1, 2)], # type: ignore[list-item]
        [[0, 0, 0], [0, 0, 1], [1, 0, 0], (1, 0, 1),
                   [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)], # type: ignore[list-item]
    ],
    [
        [Vec3D([0, 0, 2]), (1, 0, 2), [1, 1, 2], (0, 1, 2)], # type: ignore[list-item]
        [Vec3D([0, 0, 0]), (0, 0, 1), [1, 0, 0], (1, 0, 1),
                   [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)], # type: ignore[list-item]
    ],
    [
        np.array([([0, 0, 2]), (1, 0, 2), [1, 1, 2], (0, 1, 2)], dtype=np.float32),
        np.array([([0, 0, 0]), (0, 0, 1), [1, 0, 0], (1, 0, 1), [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)], dtype=np.float32), # noqa
    ],
    [
        torch.tensor([([0, 0, 2]), (1, 0, 2), [1, 1, 2], (0, 1, 2)], dtype=torch.float32),
        torch.tensor([([0, 0, 0]), (0, 0, 1), [1, 0, 0], (1, 0, 1), [1, 1, 0], (1, 1, 1), [0, 1, 0], (0, 1, 1)], dtype=torch.float32), # noqa
    ],
    # NOTE: Not legal -- non-homogeneous.
    # np.array([
    #     [([0, 0, 2]), [1, 0, 2], [1, 1, 2], [0, 1, 2]],
    #     [([0, 0, 0]), [0, 0, 1], [1, 0, 0], [1, 0, 1], [1, 1, 0], [1, 1, 1], [0, 1, 0], [0, 1, 1]],
    # ]),
]
# fmt: on


def line_strips3d_expected(obj: Any) -> Any:
    expected = none_empty_or_value(
        obj,
        [
            [[0, 0, 2], [1, 0, 2], [1, 1, 2], [0, 1, 2]],
            [[0, 0, 0], [0, 0, 1], [1, 0, 0], [1, 0, 1], [1, 1, 0], [1, 1, 1], [0, 1, 0], [0, 1, 1]],
        ],
    )
    return LineStrip3DBatch(expected)


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
        strips = cast(LineStrip3DArrayLike, strips)
        radii = cast(Optional[RadiusArrayLike], radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[InstanceKeyArrayLike], instance_keys)

        print(
            f"rr.LineStrips3D(\n"
            f"    {strips}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    class_ids={class_ids!r}\n"
            f"    instance_keys={instance_keys!r}\n"
            f")"
        )
        arch = rr.LineStrips3D(
            strips,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.strips == line_strips3d_expected(strips)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.instance_keys == instance_keys_expected(instance_keys)


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
def test_line_segments3d(data: LineStrip3DArrayLike) -> None:
    arch = rr.LineStrips3D(data)

    assert arch.strips == LineStrip3DBatch(
        [[[0, 0, 0], [0, 0, 1]], [[1, 0, 0], [1, 0, 1]], [[1, 1, 0], [1, 1, 1]], [[0, 1, 0], [0, 1, 1]]],
    )


def test_single_line_strip2d() -> None:
    # Regression test for #3643
    # Single linestrip can be passed and is not interpreted as batch of zero sized line strips.
    reference = rr.LineStrips3D([rr.components.LineStrip3D([[0, 0, 0], [1, 1, 1]])])
    assert len(reference.strips) == 1
    assert reference == rr.LineStrips3D(rr.components.LineStrip3D([[0, 0, 0], [1, 1, 1]]))
    assert reference == rr.LineStrips3D([[[0, 0, 0], [1, 1, 1]]])
    assert reference == rr.LineStrips3D([[0, 0, 0], [1, 1, 1]])
    assert reference == rr.LineStrips3D(np.array([[0, 0, 0], [1, 1, 1]]))
    assert reference == rr.LineStrips3D([np.array([0, 0, 0]), np.array([1, 1, 1])])


def test_line_strip2d_invalid_shapes() -> None:
    rr.set_strict_mode(True)

    # We used to support flat arrays but this becomes too ambiguous when passing a single strip.
    with pytest.raises(ValueError):
        rr.LineStrips3D(
            [
                [0, 0, 2, 1, 4, -1, 6, 0],
                [0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1],
            ],
        )
    # fmt: off
    with pytest.raises(ValueError):
        rr.LineStrips3D(
            [
                np.array([0, 0, 2, 1, 0, 2, 1, 1, 2, 0, 1, 2], dtype=np.float32),
                np.array([0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, ], dtype=np.float32),
            ],
        )
    # fmt: on

    # not homogeneous numpy arrays
    with pytest.raises(ValueError):
        rr.LineStrips3D(
            np.array(
                np.array(
                    [
                        [([0, 0, 2]), [1, 0, 2], [1, 1, 2], [0, 1, 2]],
                        [([0, 0, 0]), [0, 0, 1], [1, 0, 0], [1, 0, 1], [1, 1, 0], [1, 1, 1], [0, 1, 0], [0, 1, 1]],
                    ]
                ),
            )
        )
    with pytest.raises(ValueError):
        rr.LineStrips3D(
            np.array(
                [
                    [0, 0, 2, 1, 4, -1, 6, 0],
                    [0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1],
                ]
            ),
        )


if __name__ == "__main__":
    test_line_strips3d()
