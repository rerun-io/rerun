# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer13Ext" class in "affix_fuzzer13_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["AffixFuzzer13", "AffixFuzzer13ArrayLike", "AffixFuzzer13Batch", "AffixFuzzer13Like"]


@define(init=False)
class AffixFuzzer13(ComponentMixin):
    _BATCH_TYPE = None

    def __init__(self: Any, many_strings_optional: list[str] | None = None):
        """Create a new instance of the AffixFuzzer13 component."""

        # You can define your own __init__ function as a member of AffixFuzzer13Ext in affix_fuzzer13_ext.py
        self.__attrs_init__(many_strings_optional=many_strings_optional)

    many_strings_optional: list[str] | None = field(default=None)


AffixFuzzer13Like = AffixFuzzer13
AffixFuzzer13ArrayLike = Union[
    AffixFuzzer13,
    Sequence[AffixFuzzer13Like],
]


class AffixFuzzer13Batch(BaseBatch[AffixFuzzer13ArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={}))
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.testing.components.AffixFuzzer13")

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer13ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of AffixFuzzer13 not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in affix_fuzzer13_ext.py


# This is patched in late to avoid circular dependencies.
AffixFuzzer13._BATCH_TYPE = AffixFuzzer13Batch  # type: ignore[assignment]
