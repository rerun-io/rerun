# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer13Ext" class in "affix_fuzzer13_ext.py".

from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer13", "AffixFuzzer13Array", "AffixFuzzer13ArrayLike", "AffixFuzzer13Like", "AffixFuzzer13Type"]


@define
class AffixFuzzer13:
    # You can define your own __init__ function as a member of AffixFuzzer13Ext in affix_fuzzer13_ext.py

    many_strings_optional: list[str] | None = field(default=None)


AffixFuzzer13Like = AffixFuzzer13
AffixFuzzer13ArrayLike = Union[
    AffixFuzzer13,
    Sequence[AffixFuzzer13Like],
]


# --- Arrow support ---


class AffixFuzzer13Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={})),
            "rerun.testing.components.AffixFuzzer13",
        )


class AffixFuzzer13Array(BaseExtensionArray[AffixFuzzer13ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer13"
    _EXTENSION_TYPE = AffixFuzzer13Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer13ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer13_ext.py


AffixFuzzer13Type._ARRAY_TYPE = AffixFuzzer13Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer13Type())
