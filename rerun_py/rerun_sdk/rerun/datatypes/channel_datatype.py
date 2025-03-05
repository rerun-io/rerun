# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/channel_datatype.fbs".

# You can extend this class by creating a "ChannelDatatypeExt" class in "channel_datatype_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
)
from .channel_datatype_ext import ChannelDatatypeExt

__all__ = ["ChannelDatatype", "ChannelDatatypeArrayLike", "ChannelDatatypeBatch", "ChannelDatatypeLike"]


from enum import Enum


class ChannelDatatype(ChannelDatatypeExt, Enum):
    """
    **Datatype**: The innermost datatype of an image.

    How individual color channel components are encoded.
    """

    U8 = 6
    """8-bit unsigned integer."""

    I8 = 7
    """8-bit signed integer."""

    U16 = 8
    """16-bit unsigned integer."""

    I16 = 9
    """16-bit signed integer."""

    U32 = 10
    """32-bit unsigned integer."""

    I32 = 11
    """32-bit signed integer."""

    U64 = 12
    """64-bit unsigned integer."""

    I64 = 13
    """64-bit signed integer."""

    F16 = 33
    """16-bit IEEE-754 floating point, also known as `half`."""

    F32 = 34
    """32-bit IEEE-754 floating point, also known as `float` or `single`."""

    F64 = 35
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
ChannelDatatypeArrayLike = Union[
    ChannelDatatypeLike,
    Sequence[ChannelDatatypeLike],
]


class ChannelDatatypeBatch(BaseBatch[ChannelDatatypeArrayLike]):
    _ARROW_DATATYPE = pa.uint8()

    @staticmethod
    def _native_to_pa_array(data: ChannelDatatypeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (ChannelDatatype, int, str)):
            data = [data]

        pa_data = [ChannelDatatype.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
