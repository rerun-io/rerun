# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer3Ext" class in "affix_fuzzer3_ext.py".

from __future__ import annotations

from rerun._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer3", "AffixFuzzer3Batch"]


class AffixFuzzer3(datatypes.AffixFuzzer1, ComponentMixin):
    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of AffixFuzzer3Ext in affix_fuzzer3_ext.py

    # Note: there are no fields here because AffixFuzzer3 delegates to datatypes.AffixFuzzer1
    pass


class AffixFuzzer3Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.testing.components.AffixFuzzer3")


# This is patched in late to avoid circular dependencies.
AffixFuzzer3._BATCH_TYPE = AffixFuzzer3Batch  # type: ignore[assignment]
