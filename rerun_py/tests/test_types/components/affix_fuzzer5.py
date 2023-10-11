# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer5Ext" class in "affix_fuzzer5_ext.py".

from __future__ import annotations

from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer5", "AffixFuzzer5Batch", "AffixFuzzer5Type"]


class AffixFuzzer5(datatypes.AffixFuzzer1):
    # You can define your own __init__ function as a member of AffixFuzzer5Ext in affix_fuzzer5_ext.py

    # Note: there are no fields here because AffixFuzzer5 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer5Type(datatypes.AffixFuzzer1Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer5"


class AffixFuzzer5Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer5Type()
