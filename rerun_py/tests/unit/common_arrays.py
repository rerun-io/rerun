from __future__ import annotations

from typing import Any

import numpy as np
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd


def is_empty(v: object) -> bool:
    return v is None or (hasattr(v, "__len__") and len(v) == 0)  # type: ignore[arg-type]


U64_MAX_MINUS_1 = 2**64 - 2
U64_MAX = 2**64 - 1


vec2ds_arrays: list[rrd.Vec2DArrayLike] = [
    [],
    np.array([]),
    # Vec2DArrayLike: Sequence[Point2DLike]: Point2D
    [
        rrd.Vec2D([1, 2]),
        rrd.Vec2D([3, 4]),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2], dtype=np.float32),
        np.array([3, 4], dtype=np.float32),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
    [(1, 2), (3, 4)],
    # Vec2DArrayLike: Sequence[Point2DLike]: Sequence[float]
    [1, 2, 3, 4],
    # Vec2DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2], [3, 4]], dtype=np.float32),
    # Vec2DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4], dtype=np.float32),
]


def vec2ds_expected(empty: bool, type_: Any | None) -> Any:
    if type_:
        return type_.from_similar([] if empty else [[1.0, 2.0], [3.0, 4.0]])
    else:
        return rrd.Vec2DArray.from_similar([] if empty else [[1.0, 2.0], [3.0, 4.0]])


vec3ds_arrays: list[rrd.Vec3DArrayLike] = [
    [],
    np.array([]),
    # Vec3DArrayLike: Sequence[Point3DLike]: Point3D
    [
        rrd.Vec3D([1, 2, 3]),
        rrd.Vec3D([4, 5, 6]),
    ],
    # Vec3DArrayLike: Sequence[Point3DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2, 3], dtype=np.float32),
        np.array([4, 5, 6], dtype=np.float32),
    ],
    # Vec3DArrayLike: Sequence[Point3DLike]: Tuple[float, float]
    [(1, 2, 3), (4, 5, 6)],
    # Vec3DArrayLike: Sequence[Point3DLike]: Sequence[float]
    [1, 2, 3, 4, 5, 6],
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2, 3], [4, 5, 6]], dtype=np.float32),
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.float32),
]


def vec3ds_expected(empty: bool, type_: Any | None) -> Any:
    if type_:
        return type_.from_similar([] if empty else [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
    else:
        return rrd.Vec3DArray.from_similar([] if empty else [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])


radii_arrays: list[rrc.RadiusArrayLike | None] = [
    None,
    [],
    np.array([]),
    # RadiusArrayLike: Sequence[RadiusLike]: float
    [1, 10],
    # RadiusArrayLike: Sequence[RadiusLike]: Radius
    [
        rrc.Radius(1),
        rrc.Radius(10),
    ],
    # RadiusArrayLike: npt.NDArray[np.float32]
    np.array([1, 10], dtype=np.float32),
]


def radii_expected(empty: bool) -> Any:
    return rrc.RadiusArray.from_similar([] if empty else [1, 10])


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


def colors_expected(empty: bool) -> Any:
    return rrc.ColorArray.from_similar([] if empty else [0xAA0000CC, 0x00BB00DD])


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


def labels_expected(empty: bool) -> Any:
    return rrc.LabelArray.from_similar([] if empty else ["hello", "friend"])


draw_orders: list[rrc.DrawOrderLike | None] = [
    None,
    # DrawOrderLike: float
    300,
    # DrawOrderLike: DrawOrder
    rrc.DrawOrder(300),
]


def draw_order_expected(empty: bool) -> Any:
    return rrc.DrawOrderArray.from_similar([] if empty else [300])


class_ids_arrays = [
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


def class_ids_expected(empty: bool) -> Any:
    return rrc.ClassIdArray.from_similar([] if empty else [126, 127])


keypoint_ids_arrays = [
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


def keypoint_ids_expected(empty: bool) -> Any:
    return rrc.KeypointIdArray.from_similar([] if empty else [2, 3])


instance_keys_arrays = [
    [],
    np.array([]),
    # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: int
    [U64_MAX_MINUS_1, U64_MAX],
    # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: InstanceKey
    [rrc.InstanceKey(U64_MAX_MINUS_1), rrc.InstanceKey(U64_MAX)],
    # InstanceKeyArrayLike: np.NDArray[np.uint64]
    np.array([U64_MAX_MINUS_1, U64_MAX], dtype=np.uint64),
]


def instance_keys_expected(empty: bool) -> Any:
    return rrc.InstanceKeyArray.from_similar([] if empty else [U64_MAX_MINUS_1, U64_MAX])
