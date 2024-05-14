# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer9Ext" class in "affix_fuzzer9_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["AffixFuzzer9", "AffixFuzzer9ArrayLike", "AffixFuzzer9Batch", "AffixFuzzer9Like", "AffixFuzzer9Type"]


@define(init=False)
class AffixFuzzer9:
    def __init__(self: Any, single_string_required: AffixFuzzer9Like):
        """Create a new instance of the AffixFuzzer9 component."""

        # You can define your own __init__ function as a member of AffixFuzzer9Ext in affix_fuzzer9_ext.py
        self.__attrs_init__(single_string_required=single_string_required)

    single_string_required: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.single_string_required)

    def __hash__(self) -> int:
        return hash(self.single_string_required)


AffixFuzzer9Like = AffixFuzzer9
AffixFuzzer9ArrayLike = Union[
    AffixFuzzer9,
    Sequence[AffixFuzzer9Like],
]


class AffixFuzzer9Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer9"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), self._TYPE_NAME)


class AffixFuzzer9Batch(BaseBatch[AffixFuzzer9ArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer9Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer9ArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
