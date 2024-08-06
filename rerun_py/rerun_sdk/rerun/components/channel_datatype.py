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

    U8 = 1
    """8-bit unsigned integer."""

    U16 = 2
    """16-bit unsigned integer."""

    U32 = 3
    """32-bit unsigned integer."""

    U64 = 4
    """64-bit unsigned integer."""

    I8 = 5
    """8-bit signed integer."""

    I16 = 6
    """16-bit signed integer."""

    I32 = 7
    """32-bit signed integer."""

    I64 = 8
    """64-bit signed integer."""

    F16 = 9
    """16-bit IEEE-754 floating point, also known as `half`."""

    F32 = 10
    """32-bit IEEE-754 floating point, also known as `float` or `single`."""

    F64 = 11
    """64-bit IEEE-754 floating point, also known as `double`."""

    @classmethod
    def auto(cls, val: str | int | ChannelDatatype) -> ChannelDatatype:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, ChannelDatatype):
            return val
        if isinstance(val, int):
            return cls(val)
        try:
            return cls[val]
        except KeyError:
            val_lower = val.lower()
            for variant in cls:
                if variant.name.lower() == val_lower:
                    return variant
        raise ValueError(f"Cannot convert {val} to {cls.__name__}")

    def __str__(self) -> str:
        """Returns the variant name."""
        return self.name


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
    int,
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

        pa_data = [ChannelDatatype.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
