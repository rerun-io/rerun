from __future__ import annotations

from typing import Any

import numpy as np
import torch
from rerun.components import (
    ClassId,
    ClassIdBatch,
    Color,
    ColorBatch,
    DrawOrder,
    DrawOrderBatch,
    KeypointId,
    KeypointIdBatch,
    Radius,
    RadiusBatch,
    TextBatch,
)
from rerun.datatypes import (
    Angle,
    DVec2D,
    DVec2DArrayLike,
    DVec2DBatch,
    Float32ArrayLike,
    Quaternion,
    QuaternionArrayLike,
    Rgba32ArrayLike,
    RotationAxisAngle,
    RotationAxisAngleArrayLike,
    Utf8,
    Utf8ArrayLike,
    Uuid,
    UuidArrayLike,
    UVec3D,
    UVec3DArrayLike,
    UVec3DBatch,
    Vec2D,
    Vec2DArrayLike,
    Vec2DBatch,
    Vec3D,
    Vec3DArrayLike,
    Vec3DBatch,
    Vec4D,
    Vec4DArrayLike,
    Vec4DBatch,
)

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


dvec2ds_arrays: list[DVec2DArrayLike] = [
    [],
    np.array([]),
    # Vec2DArrayLike: Sequence[Point2DLike]:
    [
        DVec2D([1, 2]),
        DVec2D([3, 4]),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: npt.NDArray[np.float64]
    [
        np.array([1, 2], dtype=np.float64),
        np.array([3, 4], dtype=np.float64),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
    [(1, 2), (3, 4)],
    # Vec2DArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([(1, 2), (3, 4)], dtype=torch.float64),
    # Vec2DArrayLike: Sequence[Point2DLike]: Sequence[float]
    [1, 2, 3, 4],
    # Vec2DArrayLike: npt.NDArray[np.float64]
    np.array([[1, 2], [3, 4]], dtype=np.float64),
    # Vec2DArrayLike: npt.NDArray[np.float64]
    np.array([1, 2, 3, 4], dtype=np.float64),
    # Vec2DArrayLike: npt.NDArray[np.float64]
    np.array([1, 2, 3, 4], dtype=np.float64).reshape((2, 2, 1, 1, 1)),
    # PyTorch array
    torch.asarray([1, 2, 3, 4], dtype=torch.float64),
]


def dvec2ds_expected(obj: Any, type_: Any | None = None) -> Any:
    if type_ is None:
        type_ = DVec2DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0], [3.0, 4.0]])

    return type_._optional(expected)


vec2ds_arrays: list[Vec2DArrayLike] = [
    [],
    np.array([]),
    # Vec2DArrayLike: Sequence[Point2DLike]: Point2D
    [
        Vec2D([1, 2]),
        Vec2D([3, 4]),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2], dtype=np.float32),
        np.array([3, 4], dtype=np.float32),
    ],
    # Vec2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
    [(1, 2), (3, 4)],
    # Vec2DArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([(1, 2), (3, 4)], dtype=torch.float32),
    # Vec2DArrayLike: Sequence[Point2DLike]: Sequence[float]
    [1, 2, 3, 4],
    # Vec2DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2], [3, 4]], dtype=np.float32),
    # Vec2DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4], dtype=np.float32),
    # Vec2DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4], dtype=np.float32).reshape((2, 2, 1, 1, 1)),
    # PyTorch array
    torch.asarray([1, 2, 3, 4], dtype=torch.float32),
]


def vec2ds_expected(obj: Any, type_: Any | None = None) -> Any:
    if type_ is None:
        type_ = Vec2DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0], [3.0, 4.0]])

    return type_._optional(expected)


vec3ds_arrays: list[Vec3DArrayLike] = [
    [],
    np.array([]),
    # Vec3DArrayLike: Sequence[Position3DLike]: Position3D
    [
        Vec3D([1, 2, 3]),
        Vec3D([4, 5, 6]),
    ],
    # Vec3DArrayLike: Sequence[Position3DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2, 3], dtype=np.float32),
        np.array([4, 5, 6], dtype=np.float32),
    ],
    # Vec3DArrayLike: Sequence[Position3DLike]: Tuple[float, float]
    [(1, 2, 3), (4, 5, 6)],
    # Vec3DArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([(1, 2, 3), (4, 5, 6)], dtype=torch.float32),
    # Vec3DArrayLike: Sequence[Position3DLike]: Sequence[float]
    [1, 2, 3, 4, 5, 6],
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2, 3], [4, 5, 6]], dtype=np.float32),
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.float32),
    # Vec3DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.float32).reshape((2, 3, 1, 1, 1)),
    # PyTorch array
    torch.asarray([1, 2, 3, 4, 5, 6], dtype=torch.float32),
]


def vec3ds_expected(obj: Any, type_: Any | None = None) -> Any:
    if type_ is None:
        type_ = Vec3DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])

    return type_._optional(expected)


vec4ds_arrays: list[Vec4DArrayLike] = [
    [],
    np.array([]),
    # Vec4DArrayLike: Sequence[Position3DLike]: Position3D
    [
        Vec4D([1, 2, 3, 4]),
        Vec4D([5, 6, 7, 8]),
    ],
    # Vec4DArrayLike: Sequence[Position3DLike]: npt.NDArray[np.float32]
    [
        np.array([1, 2, 3, 4], dtype=np.float32),
        np.array([5, 6, 7, 8], dtype=np.float32),
    ],
    # Vec4DArrayLike: Sequence[Position3DLike]: Tuple[float, float]
    [(1, 2, 3, 4), (5, 6, 7, 8)],
    # Vec4DArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([(1, 2, 3, 4), (5, 6, 7, 8)], dtype=torch.float32),
    # Vec4DArrayLike: Sequence[Position3DLike]: Sequence[float]
    [1, 2, 3, 4, 5, 6, 7, 8],
    # Vec4DArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2, 3, 4], [5, 6, 7, 8]], dtype=np.float32),
    # Vec4DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6, 7, 8], dtype=np.float32),
    # Vec4DArrayLike: npt.NDArray[np.float32]
    np.array([1, 2, 3, 4, 5, 6, 7, 8], dtype=np.float32).reshape((2, 4, 1, 1, 1)),
    # PyTorch array
    torch.asarray([1, 2, 3, 4, 5, 6, 7, 8], dtype=torch.float32),
]


def vec4ds_expected(obj: Any, type_: Any | None = None) -> Any:
    if type_ is None:
        type_ = Vec4DBatch

    expected = none_empty_or_value(obj, [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]])

    return type_._optional(expected)


uvec3ds_arrays: list[UVec3DArrayLike] = [
    [],
    np.array([]),
    # UVec3DArrayLike: Sequence[Position3DLike]: Position3D
    [
        UVec3D([1, 2, 3]),
        UVec3D([4, 5, 6]),
    ],
    # UVec3DArrayLike: Sequence[Position3DLike]: npt.NDArray[np.uint32]
    [
        np.array([1, 2, 3], dtype=np.uint32),
        np.array([4, 5, 6], dtype=np.uint32),
    ],
    # UVec3DArrayLike: Sequence[Position3DLike]: Tuple[uint, uint]
    [(1, 2, 3), (4, 5, 6)],
    # UVec3DArrayLike: Sequence[Position3DLike]: Sequence[uint]
    [1, 2, 3, 4, 5, 6],
    # UVec3DArrayLike: npt.NDArray[np.uint32]
    np.array([[1, 2, 3], [4, 5, 6]], dtype=np.uint32),
    # UVec3DArrayLike: npt.NDArray[np.uint32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.uint32),
    # UVec3DArrayLike: npt.NDArray[np.uint32]
    np.array([1, 2, 3, 4, 5, 6], dtype=np.uint32).reshape((2, 3, 1, 1, 1)),
]


def uvec3ds_expected(obj: Any, type_: Any | None = None) -> Any:
    if type_ is None:
        type_ = UVec3DBatch

    expected = none_empty_or_value(obj, [[1, 2, 3], [4, 5, 6]])

    return type_._optional(expected)


quaternions_arrays: list[QuaternionArrayLike] = [
    [],
    Quaternion(xyzw=[1, 2, 3, 4]),
    Quaternion(xyzw=[1.0, 2.0, 3.0, 4.0]),
    Quaternion(xyzw=np.array([1, 2, 3, 4])),
    Quaternion(xyzw=torch.tensor([1, 2, 3, 4])),
    [
        Quaternion(xyzw=np.array([1, 2, 3, 4])),
        Quaternion(xyzw=[1, 2, 3, 4]),
    ],
    # QuaternionArrayLike: npt.NDArray[np.float32]
    np.array([[1, 2, 3, 4], [1, 2, 3, 4]], dtype=np.float32),
]


def quaternions_expected(rotations: QuaternionArrayLike, type_: Any) -> Any:
    if rotations is None:
        return type_._optional(None)
    elif hasattr(rotations, "__len__") and len(rotations) == 0:  # type: ignore[arg-type]
        return type_._optional(rotations)
    elif isinstance(rotations, Quaternion):
        return type_._optional(Quaternion(xyzw=[1, 2, 3, 4]))
    else:  # sequence of Rotation3DLike
        return type_._optional([Quaternion(xyzw=[1, 2, 3, 4])] * 2)


rotation_axis_angle_arrays: list[RotationAxisAngleArrayLike] = [
    [],
    RotationAxisAngle([1, 2, 3], 4),
    RotationAxisAngle([1.0, 2.0, 3.0], Angle(4)),
    RotationAxisAngle(Vec3D([1, 2, 3]), Angle(4)),
    RotationAxisAngle(np.array([1, 2, 3], dtype=np.uint8), Angle(rad=4)),
    RotationAxisAngle(torch.tensor([1, 2, 3]), Angle(rad=4)),
    [
        RotationAxisAngle([1, 2, 3], 4),
        RotationAxisAngle([1, 2, 3], 4),
    ],
]


def expected_rotation_axis_angles(rotations: RotationAxisAngleArrayLike, type_: Any) -> Any:
    if rotations is None:
        return type_._optional(None)
    elif hasattr(rotations, "__len__") and len(rotations) == 0:
        return type_._optional(rotations)
    elif isinstance(rotations, RotationAxisAngle):
        return type_._optional(RotationAxisAngle([1, 2, 3], 4))
    elif isinstance(rotations, Quaternion):
        return type_._optional(Quaternion(xyzw=[1, 2, 3, 4]))
    else:  # sequence of Rotation3DLike
        return type_._optional([RotationAxisAngle([1, 2, 3], 4)] * 2)


radii_arrays: list[Float32ArrayLike | None] = [
    None,
    [],
    np.array([]),
    # Float32ArrayLike: Sequence[RadiusLike]: float
    [1, 10],
    # Float32ArrayLike: Sequence[RadiusLike]: Radius
    [
        Radius(1),
        Radius(10),
    ],
    # Float32ArrayLike: npt.NDArray[np.float32]
    np.array([1, 10], dtype=np.float32),
]


def radii_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [1, 10])

    return RadiusBatch._optional(expected)


colors_arrays: list[Rgba32ArrayLike | None] = [
    None,
    [],
    np.array([]),
    # Rgba32ArrayLike: Sequence[ColorLike]: int
    [
        0xAA0000CC,
        0x00BB00DD,
    ],
    # Rgba32ArrayLike: Sequence[ColorLike]: Color
    [
        Color(0xAA0000CC),
        Color(0x00BB00DD),
    ],
    # Rgba32ArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint8]
    np.array(
        [
            [0xAA, 0x00, 0x00, 0xCC],
            [0x00, 0xBB, 0x00, 0xDD],
        ],
        dtype=np.uint8,
    ),
    # Rgba32ArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint32]
    np.array(
        [
            [0xAA0000CC],
            [0x00BB00DD],
        ],
        dtype=np.uint32,
    ),
    # Rgba32ArrayLike: Sequence[ColorLike]: npt.NDArray[np.float32]
    np.array(
        [
            [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
            [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
        ],
        dtype=np.float32,
    ),
    # Rgba32ArrayLike: Sequence[ColorLike]: npt.NDArray[np.float64]
    np.array(
        [
            [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
            [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
        ],
        dtype=np.float64,
    ),
    # Rgba32ArrayLike: torch.tensor is np.ArrayLike
    torch.tensor(
        [
            [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
            [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
        ],
        dtype=torch.float64,
    ),
    # Rgba32ArrayLike: npt.NDArray[np.uint8]
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
    # Rgba32ArrayLike: npt.NDArray[np.uint32]
    np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    ),
    # Rgba32ArrayLike: npt.NDArray[np.float32]
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
    # Rgba32ArrayLike: npt.NDArray[np.float64]
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
    return ColorBatch._optional(expected)


labels_arrays: list[Utf8ArrayLike | None] = [
    None,
    [],
    # Utf8ArrayLike: Sequence[TextLike]: str
    ["hello", "friend"],
    # Utf8ArrayLike: Sequence[TextLike]: Label
    [
        Utf8("hello"),
        Utf8("friend"),
    ],
]


def labels_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, ["hello", "friend"])
    return TextBatch._optional(expected)


draw_orders: list[Float32ArrayLike | None] = [
    None,
    # Float32ArrayLike: float
    300.0,
    # Float32ArrayLike: DrawOrder
    DrawOrder(300),
]


def draw_order_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [300])
    return DrawOrderBatch._optional(expected)


class_ids_arrays = [
    [],
    np.array([]),
    # ClassIdArrayLike: Sequence[ClassIdLike]: int
    [126, 127],
    # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
    [ClassId(126), ClassId(127)],
    # ClassIdArrayLike: np.NDArray[np.uint8]
    np.array([126, 127], dtype=np.uint8),
    # ClassIdArrayLike: np.NDArray[np.uint16]
    np.array([126, 127], dtype=np.uint16),
    # ClassIdArrayLike: np.NDArray[np.uint32]
    np.array([126, 127], dtype=np.uint32),
    # ClassIdArrayLike: np.NDArray[np.uint64]
    np.array([126, 127], dtype=np.uint64),
    # ClassIdArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([126, 127], dtype=torch.uint8),
]


def class_ids_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [126, 127])
    return ClassIdBatch._optional(expected)


keypoint_ids_arrays = [
    [],
    np.array([]),
    # KeypointIdArrayLike: Sequence[KeypointIdLike]: int
    [2, 3],
    # KeypointIdArrayLike: Sequence[KeypointIdLike]: KeypointId
    [KeypointId(2), KeypointId(3)],
    # KeypointIdArrayLike: np.NDArray[np.uint8]
    np.array([2, 3], dtype=np.uint8),
    # KeypointIdArrayLike: np.NDArray[np.uint16]
    np.array([2, 3], dtype=np.uint16),
    # KeypointIdArrayLike: np.NDArray[np.uint32]
    np.array([2, 3], dtype=np.uint32),
    # KeypointIdArrayLike: np.NDArray[np.uint64]
    np.array([2, 3], dtype=np.uint64),
    # KeypointIdArrayLike: torch.tensor is np.ArrayLike
    torch.tensor([2, 3], dtype=torch.uint8),
]


def keypoint_ids_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, [2, 3])
    return KeypointIdBatch._optional(expected)


uuid_bytes0 = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
uuid_bytes1 = [16, 17, 127, 3, 4, 255, 6, 7, 21, 9, 10, 11, 12, 0, 14, 15]

uuids_arrays: list[UuidArrayLike] = [
    [],
    np.array([]),
    # UuidArrayLike: Sequence[UuidLike]: Sequence[int]
    [uuid_bytes0, uuid_bytes1],
    # UuidArrayLike: Sequence[UuidLike]: npt.NDArray[np.uint8]
    np.array([uuid_bytes0, uuid_bytes1], dtype=np.uint8),
    # UuidArrayLike: Sequence[UuidLike]: npt.NDArray[np.uint32]
    np.array([uuid_bytes0, uuid_bytes1], dtype=np.uint32),
    # UuidArrayLike: Sequence[UuidLike]: Uuid
    [Uuid(uuid_bytes0), Uuid(uuid_bytes1)],
    # UuidArrayLike: Sequence[UuidLike]: Bytes
    [bytes(uuid_bytes0), bytes(uuid_bytes1)],
]
