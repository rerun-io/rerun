# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer9Ext" class in "affix_fuzzer9_ext.py".

from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer9", "AffixFuzzer9Array", "AffixFuzzer9ArrayLike", "AffixFuzzer9Like", "AffixFuzzer9Type"]


@define
class AffixFuzzer9:
    # You can define your own __init__ function as a member of AffixFuzzer9Ext in affix_fuzzer9_ext.py

    single_string_required: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.single_string_required)


AffixFuzzer9Like = AffixFuzzer9
AffixFuzzer9ArrayLike = Union[
    AffixFuzzer9,
    Sequence[AffixFuzzer9Like],
]


# --- Arrow support ---


class AffixFuzzer9Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.testing.components.AffixFuzzer9")


class AffixFuzzer9Array(BaseExtensionArray[AffixFuzzer9ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer9"
    _EXTENSION_TYPE = AffixFuzzer9Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer9ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer9_ext.py


AffixFuzzer9Type._ARRAY_TYPE = AffixFuzzer9Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer9Type())
