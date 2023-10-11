# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer4Ext" class in "affix_fuzzer4_ext.py".

from __future__ import annotations

from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer4", "AffixFuzzer4Batch", "AffixFuzzer4Type"]


class AffixFuzzer4(datatypes.AffixFuzzer1):
    # You can define your own __init__ function as a member of AffixFuzzer4Ext in affix_fuzzer4_ext.py

    # Note: there are no fields here because AffixFuzzer4 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer4Type(datatypes.AffixFuzzer1Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer4"


class AffixFuzzer4Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer4Type()
