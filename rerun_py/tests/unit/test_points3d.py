from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import pytest
import rerun.experimental as rr_exp
from rerun.experimental import cmp as rr_cmp
from rerun.experimental import dt as rr_dt

# TODO(cmc): roundtrips (serialize in python, deserialize in rust)

U64_MAX_MINUS_1 = 2**64 - 2
U64_MAX = 2**64 - 1


def test_points3d() -> None:
    points_arrays: list[rr_dt.Point3DArrayLike] = [
        [],
        np.array([]),
        # Point3DArrayLike: Sequence[Point3DLike]: Point3D
        [
            rr_dt.Point3D(1, 2, 3),
            rr_dt.Point3D(4, 5, 6),
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

    radii_arrays: list[rr_cmp.RadiusArrayLike | None] = [
        None,
        [],
        np.array([]),
        # RadiusArrayLike: Sequence[RadiusLike]: float
        [42, 43],
        # RadiusArrayLike: Sequence[RadiusLike]: Radius
        [
            rr_cmp.Radius(42),
            rr_cmp.Radius(43),
        ],
        # RadiusArrayLike: npt.NDArray[np.float32]
        np.array([42, 43], dtype=np.float32),
    ]

    colors_arrays: list[rr_cmp.ColorArrayLike | None] = [
        None,
        [],
        np.array([]),
        # ColorArrayLike: Sequence[ColorLike]: int
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        # ColorArrayLike: Sequence[ColorLike]: Color
        [
            rr_cmp.Color(0xAA0000CC),
            rr_cmp.Color(0x00BB00DD),
        ],
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint8]
        np.array(
            [
                [0xAA, 0x00, 0x00, 0xCC],
                [0x00, 0xBB, 0x00, 0xDD],
            ],
            dtype=np.uint8,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint32]
        np.array(
            [
                [0xAA0000CC],
                [0x00BB00DD],
            ],
            dtype=np.uint32,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.float32]
        np.array(
            [
                [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
                [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
            ],
            dtype=np.float32,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.float64]
        np.array(
            [
                [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
                [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
            ],
            dtype=np.float64,
        ),
        # ColorArrayLike: npt.NDArray[np.uint8]
        np.array(
            [
                0xAA,
                0x00,
                0x00,
                0xCC,
                0x00,
                0xBB,
                0x00,
                0xDD,
            ],
            dtype=np.uint8,
        ),
        # ColorArrayLike: npt.NDArray[np.uint32]
        np.array(
            [
                0xAA0000CC,
                0x00BB00DD,
            ],
            dtype=np.uint32,
        ),
        # ColorArrayLike: npt.NDArray[np.float32]
        np.array(
            [
                0xAA / 0xFF,
                0.0,
                0.0,
                0xCC / 0xFF,
                0.0,
                0xBB / 0xFF,
                0.0,
                0xDD / 0xFF,
            ],
            dtype=np.float32,
        ),
        # ColorArrayLike: npt.NDArray[np.float64]
        np.array(
            [
                0xAA / 0xFF,
                0.0,
                0.0,
                0xCC / 0xFF,
                0.0,
                0xBB / 0xFF,
                0.0,
                0xDD / 0xFF,
            ],
            dtype=np.float64,
        ),
    ]

    labels_arrays: list[rr_cmp.LabelArrayLike | None] = [
        None,
        [],
        # LabelArrayLike: Sequence[LabelLike]: str
        ["hello", "friend"],
        # LabelArrayLike: Sequence[LabelLike]: Label
        [
            rr_cmp.Label("hello"),
            rr_cmp.Label("friend"),
        ],
    ]

    draw_orders: list[rr_cmp.DrawOrderLike | None] = [
        None,
        # DrawOrderLike: float
        300,
        # DrawOrderLike: DrawOrder
        rr_cmp.DrawOrder(300),
    ]

    class_id_arrays = [
        [],
        np.array([]),
        # ClassIdArrayLike: Sequence[ClassIdLike]: int
        [126, 127],
        # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
        [rr_cmp.ClassId(126), rr_cmp.ClassId(127)],
        # ClassIdArrayLike: np.NDArray[np.uint8]
        np.array([126, 127], dtype=np.uint8),
        # ClassIdArrayLike: np.NDArray[np.uint16]
        np.array([126, 127], dtype=np.uint16),
        # ClassIdArrayLike: np.NDArray[np.uint32]
        np.array([126, 127], dtype=np.uint32),
        # ClassIdArrayLike: np.NDArray[np.uint64]
        np.array([126, 127], dtype=np.uint64),
    ]

    keypoint_id_arrays = [
        [],
        np.array([]),
        # KeypointIdArrayLike: Sequence[KeypointIdLike]: int
        [2, 3],
        # KeypointIdArrayLike: Sequence[KeypointIdLike]: KeypointId
        [rr_cmp.KeypointId(2), rr_cmp.KeypointId(3)],
        # KeypointIdArrayLike: np.NDArray[np.uint8]
        np.array([2, 3], dtype=np.uint8),
        # KeypointIdArrayLike: np.NDArray[np.uint16]
        np.array([2, 3], dtype=np.uint16),
        # KeypointIdArrayLike: np.NDArray[np.uint32]
        np.array([2, 3], dtype=np.uint32),
        # KeypointIdArrayLike: np.NDArray[np.uint64]
        np.array([2, 3], dtype=np.uint64),
    ]

    instance_key_arrays = [
        [],
        np.array([]),
        # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: int
        [U64_MAX_MINUS_1, U64_MAX],
        # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: InstanceKey
        [rr_cmp.InstanceKey(U64_MAX_MINUS_1), rr_cmp.InstanceKey(U64_MAX)],
        # InstanceKeyArrayLike: np.NDArray[np.uint64]
        np.array([U64_MAX_MINUS_1, U64_MAX], dtype=np.uint64),
    ]

    all_arrays = itertools.zip_longest(
        points_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        draw_orders,
        class_id_arrays,
        keypoint_id_arrays,
        instance_key_arrays,
    )

    for points, radii, colors, labels, draw_order, class_ids, keypoint_ids, instance_keys in all_arrays:
        points = points if points is not None else points_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        points = cast(Optional[rr_dt.Point3DArrayLike], points)
        radii = cast(Optional[rr_cmp.RadiusArrayLike], radii)
        colors = cast(Optional[rr_cmp.ColorArrayLike], colors)
        labels = cast(Optional[rr_cmp.LabelArrayLike], labels)
        draw_order = cast(Optional[rr_cmp.DrawOrderArrayLike], draw_order)
        class_ids = cast(Optional[rr_cmp.ClassIdArrayLike], class_ids)
        keypoint_ids = cast(Optional[rr_cmp.KeypointIdArrayLike], keypoint_ids)
        instance_keys = cast(Optional[rr_cmp.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr_exp.Points3D(\n"
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
        arch = rr_exp.Points3D(
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

        assert arch.points == rr_cmp.Point3DArray.from_similar(
            [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]] if non_empty(points) else []
        )
        assert arch.radii == rr_cmp.RadiusArray.from_similar([42, 43] if non_empty(radii) else [])
        assert arch.colors == rr_cmp.ColorArray.from_similar([0xAA0000CC, 0x00BB00DD] if non_empty(colors) else [])
        assert arch.labels == rr_cmp.LabelArray.from_similar(["hello", "friend"] if non_empty(labels) else [])
        assert arch.draw_order == rr_cmp.DrawOrderArray.from_similar([300] if draw_order is not None else [])
        assert arch.class_ids == rr_cmp.ClassIdArray.from_similar([126, 127] if non_empty(class_ids) else [])
        assert arch.keypoint_ids == rr_cmp.KeypointIdArray.from_similar([2, 3] if non_empty(keypoint_ids) else [])
        assert arch.instance_keys == rr_cmp.InstanceKeyArray.from_similar(
            [U64_MAX_MINUS_1, U64_MAX] if non_empty(instance_keys) else []
        )


def non_empty(v: object) -> bool:
    return v is not None and len(v) > 0  # type: ignore[arg-type]


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
    pts = rr_exp.Points3D(points=np.zeros((5, 3)), colors=data)

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
    pts = rr_exp.Points3D(points=np.zeros((5, 3)), colors=data)

    assert pts.colors == rr_cmp.ColorArray.from_similar(
        [
            rr_cmp.Color([0, 128, 0, 255]),
            rr_cmp.Color([128, 0, 0, 255]),
        ]
    )


if __name__ == "__main__":
    test_points3d()
