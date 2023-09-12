# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["AffixFuzzer5", "AffixFuzzer5Array", "AffixFuzzer5Type"]


class AffixFuzzer5(datatypes.AffixFuzzer1):
    # You can define your own __init__ function as a member of AffixFuzzer5Ext in affix_fuzzer5_ext.py

    # Note: there are no fields here because AffixFuzzer5 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer5Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer5"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer5Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer5"
    _EXTENSION_TYPE = AffixFuzzer5Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer5Type._ARRAY_TYPE = AffixFuzzer5Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer5Type())
