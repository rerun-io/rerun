# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer21Ext" class in "affix_fuzzer21_ext.py".

from __future__ import annotations

from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer21", "AffixFuzzer21Batch", "AffixFuzzer21Type"]


class AffixFuzzer21(datatypes.AffixFuzzer21):
    # You can define your own __init__ function as a member of AffixFuzzer21Ext in affix_fuzzer21_ext.py

    # Note: there are no fields here because AffixFuzzer21 delegates to datatypes.AffixFuzzer21
    pass


class AffixFuzzer21Type(datatypes.AffixFuzzer21Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer21"


class AffixFuzzer21Batch(datatypes.AffixFuzzer21Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer21Type()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer21Type())
