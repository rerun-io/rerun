from __future__ import annotations

from typing import TYPE_CHECKING, Iterable, cast

import pyarrow as pa

if TYPE_CHECKING:
    from ..log import ComponentBatchLike
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

    # Implement the ArchetypeLike
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..datatypes import TranslationRotationScale3D
        from . import RotationAxisAngle

        return TranslationRotationScale3D(rotation=cast(RotationAxisAngle, self)).as_component_batches()

    def num_instances(self) -> int:
        # Always a mono-component
        return 1
