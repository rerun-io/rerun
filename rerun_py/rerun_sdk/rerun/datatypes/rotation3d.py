# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/rotation3d.fbs".

# You can extend this class by creating a "Rotation3DExt" class in "rotation3d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, SupportsFloat, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from .rotation3d_ext import Rotation3DExt

__all__ = ["Rotation3D", "Rotation3DArrayLike", "Rotation3DBatch", "Rotation3DLike", "Rotation3DType"]


@define
class Rotation3D(Rotation3DExt):
    """**Datatype**: A 3D rotation."""

    # You can define your own __init__ function as a member of Rotation3DExt in rotation3d_ext.py

    inner: Union[datatypes.Quaternion, datatypes.RotationAxisAngle] = field(
        converter=Rotation3DExt.inner__field_converter_override  # type: ignore[misc]
    )
    """
    Must be one of:

    * Quaternion (datatypes.Quaternion):
        Rotation defined by a quaternion.

    * AxisAngle (datatypes.RotationAxisAngle):
        Rotation defined with an axis and an angle.
    """


if TYPE_CHECKING:
    Rotation3DLike = Union[Rotation3D, datatypes.Quaternion, datatypes.RotationAxisAngle, Sequence[SupportsFloat]]
    Rotation3DArrayLike = Union[
        Rotation3D,
        datatypes.Quaternion,
        datatypes.RotationAxisAngle,
        Sequence[Rotation3DLike],
    ]
else:
    Rotation3DLike = Any
    Rotation3DArrayLike = Any


class Rotation3DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Rotation3D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field(
                    "Quaternion",
                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4),
                    nullable=False,
                    metadata={},
                ),
                pa.field(
                    "AxisAngle",
                    pa.struct([
                        pa.field(
                            "axis",
                            pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                            nullable=False,
                            metadata={},
                        ),
                        pa.field(
                            "angle",
                            pa.dense_union([
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field("Radians", pa.float32(), nullable=False, metadata={}),
                                pa.field("Degrees", pa.float32(), nullable=False, metadata={}),
                            ]),
                            nullable=False,
                            metadata={},
                        ),
                    ]),
                    nullable=False,
                    metadata={},
                ),
            ]),
            self._TYPE_NAME,
        )


class Rotation3DBatch(BaseBatch[Rotation3DArrayLike]):
    _ARROW_TYPE = Rotation3DType()

    @staticmethod
    def _native_to_pa_array(data: Rotation3DArrayLike, data_type: pa.DataType) -> pa.Array:
        from typing import cast

        from rerun.datatypes import QuaternionBatch, RotationAxisAngleBatch

        # TODO(#2623): There should be a separate overridable `coerce_to_array` method that can be overridden.
        # If we can call iter, it may be that one of the variants implements __iter__.
        if not hasattr(data, "__iter__") or isinstance(
            data, (Rotation3D, datatypes.Quaternion, datatypes.RotationAxisAngle)
        ):  # type: ignore[arg-type]
            data = [data]  # type: ignore[list-item]
        data = cast(Sequence[Rotation3DLike], data)  # type: ignore[redundant-cast]

        types: list[int] = []
        value_offsets: list[int] = []

        num_nulls = 0
        variant_quaternion: list[datatypes.Quaternion] = []
        variant_axis_angle: list[datatypes.RotationAxisAngle] = []

        for value in data:
            if value is None:
                value_offsets.append(num_nulls)
                num_nulls += 1
                types.append(0)
            else:
                if not isinstance(value, Rotation3D):
                    value = Rotation3D(value)
                if isinstance(value.inner, datatypes.Quaternion):
                    value_offsets.append(len(variant_quaternion))
                    variant_quaternion.append(value.inner)
                    types.append(1)
                elif isinstance(value.inner, datatypes.RotationAxisAngle):
                    value_offsets.append(len(variant_axis_angle))
                    variant_axis_angle.append(value.inner)
                    types.append(2)

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
            pa.array(value_offsets, type=pa.int32()).buffers()[1],
        ]
        children = [
            pa.nulls(num_nulls),
            QuaternionBatch(variant_quaternion).as_arrow_array().storage,
            RotationAxisAngleBatch(variant_axis_angle).as_arrow_array().storage,
        ]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
