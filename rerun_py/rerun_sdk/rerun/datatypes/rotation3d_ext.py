from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import Quaternion, Rotation3D, Rotation3DArrayLike, Rotation3DLike, RotationAxisAngle

from .._unions import union_discriminant_type


class Rotation3DExt:
    """Extension for [Rotation3D][rerun.datatypes.Rotation3D]."""

    @staticmethod
    def identity() -> Rotation3D:
        from . import Quaternion, Rotation3D

        return Rotation3D(Quaternion.identity())

    @staticmethod
    def inner__field_converter_override(
        data: Rotation3DLike,
    ) -> Quaternion | RotationAxisAngle:
        from . import Quaternion, Rotation3D, RotationAxisAngle

        if isinstance(data, Rotation3D):
            return data.inner
        elif isinstance(data, (Quaternion, RotationAxisAngle)):
            return data
        else:
            return Quaternion(xyzw=np.array(data))

    @staticmethod
    def native_to_pa_array_override(data: Rotation3DArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Quaternion, QuaternionBatch, Rotation3D, RotationAxisAngle, RotationAxisAngleBatch

        if isinstance(data, Rotation3D) or isinstance(data, RotationAxisAngle) or isinstance(data, Quaternion):
            data = [data]

        types: list[int] = []
        value_offsets: list[int] = []

        num_nulls = 0
        rotation_axis_angles: list[RotationAxisAngle] = []
        quaternions: list[Quaternion] = []

        null_type_idx = 0
        quaternion_type_idx = 1
        rotation_axis_angle_type_idx = 2

        for rotation in data:
            if rotation is None:
                value_offsets.append(num_nulls)
                num_nulls += 1
                types.append(null_type_idx)
            else:
                rotation_arm = Rotation3DExt.inner__field_converter_override(rotation)

                if isinstance(rotation_arm, RotationAxisAngle):
                    value_offsets.append(len(rotation_axis_angles))
                    rotation_axis_angles.append(rotation_arm)
                    types.append(rotation_axis_angle_type_idx)
                elif isinstance(rotation_arm, Quaternion):
                    value_offsets.append(len(quaternions))
                    quaternions.append(rotation_arm)
                    types.append(quaternion_type_idx)
                else:
                    raise ValueError(
                        f"Unknown 3d rotation representation: {rotation_arm} (expected `Rotation3D`, `RotationAxisAngle`, "
                        "`Quaternion`, or `None`."
                    )
        # don't use pa.UnionArray.from_dense because it makes all fields nullable.
        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=[
                None,
                pa.array(types, type=pa.int8()).buffers()[1],
                pa.array(value_offsets, type=pa.int32()).buffers()[1],
            ],
            children=[
                pa.nulls(num_nulls, pa.null()),
                QuaternionBatch._native_to_pa_array(quaternions, union_discriminant_type(data_type, "Quaternion")),
                RotationAxisAngleBatch._native_to_pa_array(
                    rotation_axis_angles, union_discriminant_type(data_type, "AxisAngle")
                ),
            ],
        )


def is_sequence(obj: Any) -> bool:
    t = type(obj)
    return hasattr(t, "__len__") and hasattr(t, "__getitem__")
