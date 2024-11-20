# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer22Ext" class in "affix_fuzzer22_ext.py".

from __future__ import annotations

from rerun._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer22", "AffixFuzzer22Batch"]


class AffixFuzzer22(datatypes.AffixFuzzer22, ComponentMixin):
    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of AffixFuzzer22Ext in affix_fuzzer22_ext.py

    # Note: there are no fields here because AffixFuzzer22 delegates to datatypes.AffixFuzzer22
    pass


class AffixFuzzer22Batch(datatypes.AffixFuzzer22Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.testing.components.AffixFuzzer22"


# This is patched in late to avoid circular dependencies.
AffixFuzzer22._BATCH_TYPE = AffixFuzzer22Batch  # type: ignore[assignment]
