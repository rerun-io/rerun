# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer5Ext" class in "affix_fuzzer5_ext.py".

from __future__ import annotations

from rerun._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer5", "AffixFuzzer5Batch"]


class AffixFuzzer5(datatypes.AffixFuzzer1, ComponentMixin):
    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of AffixFuzzer5Ext in affix_fuzzer5_ext.py

    # Note: there are no fields here because AffixFuzzer5 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer5Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.testing.components.AffixFuzzer5"


# This is patched in late to avoid circular dependencies.
AffixFuzzer5._BATCH_TYPE = AffixFuzzer5Batch  # type: ignore[assignment]
