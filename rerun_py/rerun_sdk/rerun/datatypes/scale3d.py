# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/scale3d.fbs".

# You can extend this class by creating a "Scale3DExt" class in "scale3d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import BaseBatch, BaseExtensionType
from .scale3d_ext import Scale3DExt

__all__ = ["Scale3D", "Scale3DArrayLike", "Scale3DBatch", "Scale3DLike", "Scale3DType"]


@define
class Scale3D(Scale3DExt):
    """
    **Datatype**: 3D scaling factor, part of a transform representation.

    Example
    -------
    ```python
    # uniform scaling
    scale = rr.datatypes.Scale3D(3.)

    # non-uniform scaling
    scale = rr.datatypes.Scale3D([1, 1, -1])
    scale = rr.datatypes.Scale3D(rr.datatypes.Vec3D([1, 1, -1]))
    ```

    """

    # You can define your own __init__ function as a member of Scale3DExt in scale3d_ext.py

    inner: Union[datatypes.Vec3D, float] = field(
        converter=Scale3DExt.inner__field_converter_override  # type: ignore[misc]
    )
    """
    Must be one of:

    * ThreeD (datatypes.Vec3D):
        Individual scaling factors for each axis, distorting the original object.

    * Uniform (float):
        Uniform scaling factor along all axis.
    """


if TYPE_CHECKING:
    Scale3DLike = Union[Scale3D, datatypes.Vec3D, float, datatypes.Vec3DLike]
    Scale3DArrayLike = Union[
        Scale3D,
        datatypes.Vec3D,
        float,
        Sequence[Scale3DLike],
    ]
else:
    Scale3DLike = Any
    Scale3DArrayLike = Any


class Scale3DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Scale3D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field(
                    "ThreeD",
                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                    nullable=False,
                    metadata={},
                ),
                pa.field("Uniform", pa.float32(), nullable=False, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class Scale3DBatch(BaseBatch[Scale3DArrayLike]):
    _ARROW_TYPE = Scale3DType()

    @staticmethod
    def _native_to_pa_array(data: Scale3DArrayLike, data_type: pa.DataType) -> pa.Array:
        from typing import cast

        from rerun.datatypes import Vec3DBatch

        # Ensure data is iterable.
        try:
            iter(data)  # type: ignore[arg-type]
        except TypeError:
            data = [data]  # type: ignore[list-item]
        data = cast(Sequence[Scale3DLike], data)

        types: list[int] = []
        value_offsets: list[int] = []

        num_nulls = 0
        variant_three_d: list[datatypes.Vec3D] = []
        variant_uniform: list[float] = []

        for value in data:
            if value is None:
                value_offsets.append(num_nulls)
                num_nulls += 1
                types.append(0)
            else:
                if not isinstance(value, Scale3D):
                    value = Scale3D(value)
                if isinstance(value.inner, datatypes.Vec3D):
                    value_offsets.append(len(variant_three_d))
                    variant_three_d.append(value.inner)
                    types.append(1)
                elif isinstance(value.inner, float):
                    value_offsets.append(len(variant_uniform))
                    variant_uniform.append(value.inner)
                    types.append(2)

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
            pa.array(value_offsets, type=pa.int32()).buffers()[1],
        ]
        children = [
            pa.nulls(num_nulls),
            Vec3DBatch(variant_three_d).as_arrow_array().storage,
            pa.array(variant_uniform, type=pa.float32()),
        ]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
