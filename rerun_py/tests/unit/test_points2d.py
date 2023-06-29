from __future__ import annotations

import itertools

import numpy as np
import rerun as rr
from rerun._rerun2 import components as rrc

# NOTE: uncomment these to get a better auto-completion experience...
# from rerun_sdk import rerun2 as rr
# from rerun_sdk.rerun2 import components as rrc

# TODO(cmc): roundtrips (serialize in python, deserialize in rust)

U64_MAX_MINUS_1 = 2**64 - 2
U64_MAX = 2**64 - 1


def test_points2d() -> None:
    points_arrays: list[rrc.Point2DArrayLike] = [
        [],
        np.array([]),
        # Point2DArrayLike: Sequence[Point2DLike]: Point2D
        [
            rrc.Point2D(1, 2),
            rrc.Point2D(3, 4),
        ],
        # Point2DArrayLike: Sequence[Point2DLike]: npt.NDArray[np.float32]
        [
            np.array([1, 2], dtype=np.float32),
            np.array([3, 4], dtype=np.float32),
        ],
        # Point2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
        [(1, 2), (3, 4)],
        # Point2DArrayLike: Sequence[Point2DLike]: Sequence[float]
        [1, 2, 3, 4],
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([[1, 2], [3, 4]], dtype=np.float32),
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([1, 2, 3, 4], dtype=np.float32),
    ]

    radii_arrays: list[rrc.RadiusArrayLike | None] = [
        None,
        [],
        np.array([]),
        # RadiusArrayLike: Sequence[RadiusLike]: float
        [42, 43],
        # RadiusArrayLike: Sequence[RadiusLike]: Radius
        [
            rrc.Radius(42),
            rrc.Radius(43),
        ],
        # RadiusArrayLike: npt.NDArray[np.float32]
        np.array([42, 43], dtype=np.float32),
    ]

    colors_arrays: list[rrc.ColorArrayLike | None] = [
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
            rrc.Color(0xAA0000CC),
            rrc.Color(0x00BB00DD),
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

    labels_arrays: list[rrc.LabelArrayLike | None] = [
        None,
        [],
        # LabelArrayLike: Sequence[LabelLike]: str
        ["hello", "friend"],
        # LabelArrayLike: Sequence[LabelLike]: Label
        [
            rrc.Label("hello"),
            rrc.Label("friend"),
        ],
    ]

    draw_orders: list[rrc.DrawOrderLike | None] = [
        None,
        # DrawOrderLike: float
        300,
        # DrawOrderLike: DrawOrder
        rrc.DrawOrder(300),
    ]

    class_id_arrays = [
        [],
        np.array([]),
        # ClassIdArrayLike: Sequence[ClassIdLike]: int
        [126, 127],
        # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
        [rrc.ClassId(126), rrc.ClassId(127)],
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
        [rrc.KeypointId(2), rrc.KeypointId(3)],
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
        [rrc.InstanceKey(U64_MAX_MINUS_1), rrc.InstanceKey(U64_MAX)],
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

        print(
            f"rr.Points2D(\n"
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
        arch = rr.Points2D(
            points,  # type: ignore[arg-type]
            radii=radii,  # type: ignore[arg-type]
            colors=colors,  # type: ignore[arg-type]
            labels=labels,  # type: ignore[arg-type]
            draw_order=draw_order,  # type: ignore[arg-type]
            class_ids=class_ids,  # type: ignore[arg-type]
            keypoint_ids=keypoint_ids,  # type: ignore[arg-type]
            instance_keys=instance_keys,  # type: ignore[arg-type]
        )
        print(f"{arch}\n")

        assert arch.points == rrc.Point2DArray.from_similar([[1.0, 2.0], [3.0, 4.0]] if non_empty(points) else [])
        assert arch.radii == rrc.RadiusArray.from_similar([42, 43] if non_empty(radii) else [])
        assert arch.colors == rrc.ColorArray.from_similar([0xAA0000CC, 0x00BB00DD] if non_empty(colors) else [])
        assert arch.labels == rrc.LabelArray.from_similar(["hello", "friend"] if non_empty(labels) else [])
        assert arch.draw_order == rrc.DrawOrderArray.from_similar([300] if draw_order is not None else [])
        assert arch.class_ids == rrc.ClassIdArray.from_similar([126, 127] if non_empty(class_ids) else [])
        assert arch.keypoint_ids == rrc.KeypointIdArray.from_similar([2, 3] if non_empty(keypoint_ids) else [])
        assert arch.instance_keys == rrc.InstanceKeyArray.from_similar(
            [U64_MAX_MINUS_1, U64_MAX] if non_empty(instance_keys) else []
        )


def non_empty(v: object) -> bool:
    return v is not None and len(v) > 0  # type: ignore[arg-type]


if __name__ == "__main__":
    test_points2d()
