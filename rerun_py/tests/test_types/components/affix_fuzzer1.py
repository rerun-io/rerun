# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer1Ext" class in "affix_fuzzer1_ext.py".

from __future__ import annotations

from rerun._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer1", "AffixFuzzer1Batch"]


class AffixFuzzer1(datatypes.AffixFuzzer1, ComponentMixin):
    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of AffixFuzzer1Ext in affix_fuzzer1_ext.py

    # Note: there are no fields here because AffixFuzzer1 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer1Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.testing.components.AffixFuzzer1"


# This is patched in late to avoid circular dependencies.
AffixFuzzer1._BATCH_TYPE = AffixFuzzer1Batch  # type: ignore[assignment]
