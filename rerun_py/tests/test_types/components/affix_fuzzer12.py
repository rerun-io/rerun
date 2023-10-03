# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer12Ext" class in "affix_fuzzer12_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["AffixFuzzer12", "AffixFuzzer12ArrayLike", "AffixFuzzer12Batch", "AffixFuzzer12Like", "AffixFuzzer12Type"]


@define(init=False)
class AffixFuzzer12:
    def __init__(self: Any, many_strings_required: AffixFuzzer12Like):
        """Create a new instance of the AffixFuzzer12 component."""

        # You can define your own __init__ function as a member of AffixFuzzer12Ext in affix_fuzzer12_ext.py
        self.__attrs_init__(many_strings_required=many_strings_required)

    many_strings_required: list[str] = field()


AffixFuzzer12Like = AffixFuzzer12
AffixFuzzer12ArrayLike = Union[
    AffixFuzzer12,
    Sequence[AffixFuzzer12Like],
]


class AffixFuzzer12Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer12"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={})), self._TYPE_NAME
        )


class AffixFuzzer12Batch(BaseBatch[AffixFuzzer12ArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer12Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer12ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer12_ext.py
