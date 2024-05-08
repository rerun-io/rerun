# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/angle.fbs".

# You can extend this class by creating a "AngleExt" class in "angle_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .angle_ext import AngleExt

__all__ = ["Angle", "AngleArrayLike", "AngleBatch", "AngleLike", "AngleType"]


@define(init=False)
class Angle(AngleExt):
    """**Datatype**: Angle in either radians or degrees."""

    # __init__ can be found in angle_ext.py

    inner: float = field(converter=float)
    """
    Must be one of:

    * Radians (float):
        Angle in radians. One turn is equal to 2π (or τ) radians.
        Only one of `degrees` or `radians` should be set.

    * Degrees (float):
        Angle in degrees. One turn is equal to 360 degrees.
        Only one of `degrees` or `radians` should be set.
    """

    kind: Literal["radians", "degrees"] = field(default="radians")
    """
    Possible values:

    * "radians":
        Angle in radians. One turn is equal to 2π (or τ) radians.
        Only one of `degrees` or `radians` should be set.

    * "degrees":
        Angle in degrees. One turn is equal to 360 degrees.
        Only one of `degrees` or `radians` should be set.
    """


if TYPE_CHECKING:
    AngleLike = Union[
        Angle,
        float,
    ]
    AngleArrayLike = Union[
        Angle,
        float,
        Sequence[AngleLike],
    ]
else:
    AngleLike = Any
    AngleArrayLike = Any


class AngleType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Angle"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("Radians", pa.float32(), nullable=False, metadata={}),
                pa.field("Degrees", pa.float32(), nullable=False, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class AngleBatch(BaseBatch[AngleArrayLike]):
    _ARROW_TYPE = AngleType()

    @staticmethod
    def _native_to_pa_array(data: AngleArrayLike, data_type: pa.DataType) -> pa.Array:
        # TODO(#2623): There should be a separate overridable `coerce_to_array` method that can be overridden.
        if not hasattr(data, "__iter__") or isinstance(
            data, (Angle, float)
        ):  # If we can call iter, it may be that one of the variants implements __iter__.
            data = [data]

        types: list[int] = []
        value_offsets: list[int] = []

        num_nulls = 0
        variant_radians: list[float] = []
        variant_degrees: list[float] = []

        for value in data:
            if value is None:
                value_offsets.append(num_nulls)
                num_nulls += 1
                types.append(0)
            else:
                if not isinstance(value, Angle):
                    value = Angle(value)
                if value.kind == "radians":
                    value_offsets.append(len(variant_radians))
                    variant_radians.append(value.inner)  # type: ignore[arg-type]
                    types.append(1)
                elif value.kind == "degrees":
                    value_offsets.append(len(variant_degrees))
                    variant_degrees.append(value.inner)  # type: ignore[arg-type]
                    types.append(2)

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
            pa.array(value_offsets, type=pa.int32()).buffers()[1],
        ]
        children = [
            pa.nulls(num_nulls),
            pa.array(variant_radians, type=pa.float32()),
            pa.array(variant_degrees, type=pa.float32()),
        ]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
