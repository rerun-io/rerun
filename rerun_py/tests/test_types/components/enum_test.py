# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/enum_test.fbs".

# You can extend this class by creating a "EnumTestExt" class in "enum_test_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa
from rerun._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["EnumTest", "EnumTestArrayLike", "EnumTestBatch", "EnumTestLike", "EnumTestType"]


from enum import Enum


class EnumTest(Enum):
    """**Component**: A test of the enum type."""

    Up = 0
    """Great film."""

    Down = 1
    """Feeling blue."""

    Right = 2
    """Correct."""

    Left = 3
    """It's what's remaining."""

    Forward = 4
    """It's the only way to go."""

    Back = 5
    """Baby's got it."""

    @classmethod
    def auto(cls, val: str | int | EnumTest) -> EnumTest:
        """Best-effort converter."""
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


class EnumTestType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.EnumTest"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class EnumTestBatch(BaseBatch[EnumTestArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = EnumTestType()

    @staticmethod
    def _native_to_pa_array(data: EnumTestArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (EnumTest, int, str)):
            data = [data]

        pa_data = [EnumTest.auto(v).value if v else None for v in data]

        return pa.array(pa_data, type=data_type)
