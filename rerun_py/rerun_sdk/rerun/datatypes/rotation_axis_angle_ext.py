from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pyarrow as pa

if TYPE_CHECKING:
    from . import Angle, AngleLike, RotationAxisAngleArrayLike, Vec3DLike


class RotationAxisAngleExt:
    def __init__(
        self: Any,
        axis: Vec3DLike,
        angle: AngleLike | None = None,
        *,
        radians: float | None = None,
        degrees: float | None = None,
    ) -> None:
        from . import Angle

        if len([x for x in (angle, radians, degrees) if x is not None]) != 1:
            raise ValueError("Must provide exactly one of angle, radians, or degrees")
        if radians is not None:
            angle = Angle(rad=radians)
        if degrees is not None:
            angle = Angle(deg=degrees)
        self.__attrs_init__(axis=axis, angle=angle)

    # needed because the default converter doesn't handle well Angle, which has an overridden __init__
    @staticmethod
    def angle__field_converter_override(x: AngleLike) -> Angle:
        from . import Angle

        if isinstance(x, Angle):
            return x
        else:
            return Angle(rad=x)

    @staticmethod
    def native_to_pa_array_override(data: RotationAxisAngleArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import AngleBatch, RotationAxisAngle, Vec3DBatch

        if isinstance(data, RotationAxisAngle):
            data = [data]

        axis_pa_array = Vec3DBatch._native_to_pa_array([rotation.axis for rotation in data], data_type["axis"].type)
        angle_pa_arr = AngleBatch._native_to_pa_array([rotation.angle for rotation in data], data_type["angle"].type)

        return pa.StructArray.from_arrays(
            [
                axis_pa_array,
                angle_pa_arr,
            ],
            fields=list(data_type),
        )
