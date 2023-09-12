# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".


from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    str_or_none,
)

__all__ = ["AffixFuzzer10", "AffixFuzzer10Array", "AffixFuzzer10ArrayLike", "AffixFuzzer10Like", "AffixFuzzer10Type"]


@define
class AffixFuzzer10:
    # You can define your own __init__ function as a member of AffixFuzzer10Ext in affix_fuzzer10_ext.py

    single_string_optional: str | None = field(default=None, converter=str_or_none)  # type: ignore[misc]


AffixFuzzer10Like = AffixFuzzer10
AffixFuzzer10ArrayLike = Union[
    AffixFuzzer10,
    Sequence[AffixFuzzer10Like],
]


# --- Arrow support ---


class AffixFuzzer10Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.testing.components.AffixFuzzer10")


class AffixFuzzer10Array(BaseExtensionArray[AffixFuzzer10ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer10"
    _EXTENSION_TYPE = AffixFuzzer10Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer10ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer10_ext.py


AffixFuzzer10Type._ARRAY_TYPE = AffixFuzzer10Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer10Type())
