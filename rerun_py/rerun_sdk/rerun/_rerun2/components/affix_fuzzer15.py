# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer15Ext" class in "affix_fuzzer15_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["AffixFuzzer15Array", "AffixFuzzer15Type"]


class AffixFuzzer15Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer15"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer3Type


class AffixFuzzer15Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer3ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer15"
    _EXTENSION_TYPE = AffixFuzzer15Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer3Array


AffixFuzzer15Type._ARRAY_TYPE = AffixFuzzer15Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer15Type())
