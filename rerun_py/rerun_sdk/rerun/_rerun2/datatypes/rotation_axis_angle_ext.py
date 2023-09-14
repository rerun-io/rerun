from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import Angle, AngleLike, RotationAxisAngleArrayLike


class RotationAxisAngleExt:
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
        from . import AngleArray, RotationAxisAngle, Vec3DArray

        if isinstance(data, RotationAxisAngle):
            data = [data]

        axis_pa_array = Vec3DArray._native_to_pa_array([rotation.axis for rotation in data], data_type["axis"].type)
        angle_pa_arr = AngleArray._native_to_pa_array([rotation.angle for rotation in data], data_type["angle"].type)

        return pa.StructArray.from_arrays(
            [
                axis_pa_array,
                angle_pa_arr,
            ],
            fields=list(data_type),
        )
