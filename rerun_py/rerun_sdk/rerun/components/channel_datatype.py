# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/channel_datatype.fbs".

# You can extend this class by creating a "ChannelDatatypeExt" class in "channel_datatype_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)
from .channel_datatype_ext import ChannelDatatypeExt

__all__ = [
    "ChannelDatatype",
    "ChannelDatatypeArrayLike",
    "ChannelDatatypeBatch",
    "ChannelDatatypeLike",
    "ChannelDatatypeType",
]


from enum import Enum


class ChannelDatatype(ChannelDatatypeExt, Enum):
    """
    **Component**: The innermost datatype of an image.

    How individual color channel components are encoded.
    """

    U8 = 0
    """8-bit unsigned integer."""

    U16 = 1
    """16-bit unsigned integer."""

    U32 = 2
    """32-bit unsigned integer."""

    U64 = 3
    """64-bit unsigned integer."""

    I8 = 4
    """8-bit signed integer."""

    I16 = 5
    """16-bit signed integer."""

    I32 = 6
    """32-bit signed integer."""

    I64 = 7
    """64-bit signed integer."""

    F16 = 8
    """16-bit IEEE-754 floating point, also known as `half`."""

    F32 = 9
    """32-bit IEEE-754 floating point, also known as `float` or `single`."""

    F64 = 10
    """64-bit IEEE-754 floating point, also known as `double`."""

    def __str__(self) -> str:
        """Returns the variant name."""
        if self == ChannelDatatype.U8:
            return "U8"
        elif self == ChannelDatatype.U16:
            return "U16"
        elif self == ChannelDatatype.U32:
            return "U32"
        elif self == ChannelDatatype.U64:
            return "U64"
        elif self == ChannelDatatype.I8:
            return "I8"
        elif self == ChannelDatatype.I16:
            return "I16"
        elif self == ChannelDatatype.I32:
            return "I32"
        elif self == ChannelDatatype.I64:
            return "I64"
        elif self == ChannelDatatype.F16:
            return "F16"
        elif self == ChannelDatatype.F32:
            return "F32"
        elif self == ChannelDatatype.F64:
            return "F64"
        else:
            raise ValueError("Unknown enum variant")


ChannelDatatypeLike = Union[
    ChannelDatatype,
    Literal[
        "F16",
        "F32",
        "F64",
        "I16",
        "I32",
        "I64",
        "I8",
        "U16",
        "U32",
        "U64",
        "U8",
        "f16",
        "f32",
        "f64",
        "i16",
        "i32",
        "i64",
        "i8",
        "u16",
        "u32",
        "u64",
        "u8",
    ],
]
ChannelDatatypeArrayLike = Union[ChannelDatatypeLike, Sequence[ChannelDatatypeLike]]


class ChannelDatatypeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.ChannelDatatype"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class ChannelDatatypeBatch(BaseBatch[ChannelDatatypeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ChannelDatatypeType()

    @staticmethod
    def _native_to_pa_array(data: ChannelDatatypeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (ChannelDatatype, int, str)):
            data = [data]

        data = [ChannelDatatype(v) if isinstance(v, int) else v for v in data]
        data = [ChannelDatatype[v.upper()] if isinstance(v, str) else v for v in data]
        pa_data = [v.value for v in data]

        return pa.array(pa_data, type=data_type)
