from __future__ import annotations

from typing import Any

import numpy as np
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

U64_MAX_MINUS_1 = 2**64 - 2
U64_MAX = 2**64 - 1


def none_empty_or_value(obj: Any, value: Any) -> Any:
    """
    Helper function to make value align with None / Empty types.

    If obj is None or an empty list, it is returned. Otherwise value
    is returned. This is useful for creating the `_expected` functions.
    """

    if obj is None:
        return None
    elif hasattr(obj, "__len__") and len(obj) == 0:
        return []
    else:
        return value


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


def vec2ds_expected(obj: Any, type_: Any | None) -> Any:
    if type_ is None:
        type_ = rrd.Vec2DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0], [3.0, 4.0]])

    return type_._optional(expected)


vec3ds_arrays: list[rrd.Vec3DArrayLike] = [
    [],
    np.array([]),
    # Vec3DArrayLike: Sequence[Position3DLike]: Position3D
    [
        rrd.Vec3D([1, 2, 3]),
        rrd.Vec3D([4, 5, 6]),
    ],
    # Vec3DArrayLike: Sequence[Position3DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2, 3], dtype=np.float32),
        np.array([4, 5, 6], dtype=np.float32),
    ],
    # Vec3DArrayLike: Sequence[Position3DLike]: Tuple[float, float]
    [(1, 2, 3), (4, 5, 6)],
    # Vec3DArrayLike: Sequence[Position3DLike]: Sequence[float]
    [1, 2, 3, 4, 5, 6],
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2, 3], [4, 5, 6]], dtype=np.float32),
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.float32),
]


def vec3ds_expected(obj: Any, type_: Any | None) -> Any:
    if type_ is None:
        type_ = rrd.Vec3DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])

    return type_._optional(expected)


rotations_arrays: list[rrd.Rotation3DArrayLike] = [
    [],
    # Rotation3D
    rrd.Rotation3D(rrd.Quaternion(xyzw=[1, 2, 3, 4])),
    rrd.Rotation3D(rrd.RotationAxisAngle([1.0, 2.0, 3.0], rrd.Angle(4))),
    # Quaternion
    rrd.Quaternion(xyzw=[1, 2, 3, 4]),
    rrd.Quaternion(xyzw=[1.0, 2.0, 3.0, 4.0]),
    rrd.Quaternion(xyzw=np.array([1, 2, 3, 4])),
    # RotationAxisAngle
    rrd.RotationAxisAngle([1, 2, 3], 4),
    rrd.RotationAxisAngle([1.0, 2.0, 3.0], rrd.Angle(4)),
    rrd.RotationAxisAngle(rrd.Vec3D([1, 2, 3]), rrd.Angle(4)),
    rrd.RotationAxisAngle(np.array([1, 2, 3], dtype=np.uint8), rrd.Angle(rad=4)),
    # Sequence[Rotation3DBatch]
    [
        rrd.Rotation3D(rrd.Quaternion(xyzw=[1, 2, 3, 4])),
        [1, 2, 3, 4],
        rrd.Quaternion(xyzw=[1, 2, 3, 4]),
        rrd.RotationAxisAngle([1, 2, 3], 4),
    ],
]


def expected_rotations(rotations: rrd.Rotation3DArrayLike, type_: Any) -> Any:
    if rotations is None:
        return type_._optional(None)
    elif hasattr(rotations, "__len__") and len(rotations) == 0:
        return type_._optional(rotations)
    elif isinstance(rotations, rrd.Rotation3D):
        return type_._optional(rotations)
    elif isinstance(rotations, rrd.RotationAxisAngle):
        return type_._optional(rrd.RotationAxisAngle([1, 2, 3], 4))
    elif isinstance(rotations, rrd.Quaternion):
        return type_._optional(rrd.Quaternion(xyzw=[1, 2, 3, 4]))
    else:  # sequence of Rotation3DLike
        return type_._optional([rrd.Quaternion(xyzw=[1, 2, 3, 4])] * 3 + [rrd.RotationAxisAngle([1, 2, 3], 4)])


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


def radii_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [1, 10])

    return rrc.RadiusBatch._optional(expected)


colors_arrays: list[rrd.ColorArrayLike | None] = [
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
        rrd.Color(0xAA0000CC),
        rrd.Color(0x00BB00DD),
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


def colors_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [0xAA0000CC, 0x00BB00DD])
    return rrc.ColorBatch._optional(expected)


labels_arrays: list[rrd.Utf8ArrayLike | None] = [
    None,
    [],
    # Utf8ArrayLike: Sequence[TextLike]: str
    ["hello", "friend"],
    # Utf8ArrayLike: Sequence[TextLike]: Label
    [
        rrd.Utf8("hello"),
        rrd.Utf8("friend"),
    ],
]


def labels_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, ["hello", "friend"])
    return rrc.TextBatch._optional(expected)


draw_orders: list[rrc.DrawOrderLike | None] = [
    None,
    # DrawOrderLike: float
    300,
    # DrawOrderLike: DrawOrder
    rrc.DrawOrder(300),
]


def draw_order_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [300])
    return rrc.DrawOrderBatch._optional(expected)


class_ids_arrays = [
    [],
    np.array([]),
    # ClassIdArrayLike: Sequence[ClassIdLike]: int
    [126, 127],
    # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
    [rrd.ClassId(126), rrd.ClassId(127)],
    # ClassIdArrayLike: np.NDArray[np.uint8]
    np.array([126, 127], dtype=np.uint8),
    # ClassIdArrayLike: np.NDArray[np.uint16]
    np.array([126, 127], dtype=np.uint16),
    # ClassIdArrayLike: np.NDArray[np.uint32]
    np.array([126, 127], dtype=np.uint32),
    # ClassIdArrayLike: np.NDArray[np.uint64]
    np.array([126, 127], dtype=np.uint64),
]


def class_ids_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [126, 127])
    return rrc.ClassIdBatch._optional(expected)


keypoint_ids_arrays = [
    [],
    np.array([]),
    # KeypointIdArrayLike: Sequence[KeypointIdLike]: int
    [2, 3],
    # KeypointIdArrayLike: Sequence[KeypointIdLike]: KeypointId
    [rrd.KeypointId(2), rrd.KeypointId(3)],
    # KeypointIdArrayLike: np.NDArray[np.uint8]
    np.array([2, 3], dtype=np.uint8),
    # KeypointIdArrayLike: np.NDArray[np.uint16]
    np.array([2, 3], dtype=np.uint16),
    # KeypointIdArrayLike: np.NDArray[np.uint32]
    np.array([2, 3], dtype=np.uint32),
    # KeypointIdArrayLike: np.NDArray[np.uint64]
    np.array([2, 3], dtype=np.uint64),
]


def keypoint_ids_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [2, 3])
    return rrc.KeypointIdBatch._optional(expected)


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


def instance_keys_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [U64_MAX_MINUS_1, U64_MAX])
    return rrc.InstanceKeyBatch._optional(expected)
