# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/enum_test.fbs".

# You can extend this class by creating a "EnumTestExt" class in "enum_test_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal, Union

import pyarrow as pa
from rerun._baseclasses import (
    BaseBatch,
)

__all__ = ["EnumTest", "EnumTestArrayLike", "EnumTestBatch", "EnumTestLike"]


from enum import Enum


class EnumTest(Enum):
    """**Datatype**: A test of the enum type."""

    Up = 1
    """Great film."""

    Down = 2
    """Feeling blue."""

    Right = 3
    """Correct."""

    Left = 4
    """It's what's remaining."""

    Forward = 5
    """It's the only way to go."""

    Back = 6
    """Baby's got it."""

    @classmethod
    def auto(cls, val: str | int | EnumTest) -> EnumTest:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, EnumTest):
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


EnumTestLike = Union[
    EnumTest,
    Literal["Back", "Down", "Forward", "Left", "Right", "Up", "back", "down", "forward", "left", "right", "up"],
    int,
]
EnumTestArrayLike = Union[EnumTestLike, Sequence[EnumTestLike]]


class EnumTestBatch(BaseBatch[EnumTestArrayLike]):
    _ARROW_DATATYPE = pa.uint8()

    @staticmethod
    def _native_to_pa_array(data: EnumTestArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (EnumTest, int, str)):
            data = [data]

        pa_data = [EnumTest.auto(v).value if v is not None else None for v in data]  # type: ignore[redundant-expr]

        return pa.array(pa_data, type=data_type)
