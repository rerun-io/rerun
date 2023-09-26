# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer15Ext" class in "affix_fuzzer15_ext.py".

from __future__ import annotations

from typing import Any

from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer15", "AffixFuzzer15Batch", "AffixFuzzer15Type"]


class AffixFuzzer15(datatypes.AffixFuzzer3):
    def __init__(self: Any, inner: datatypes.AffixFuzzer3Like | None = None):
        """Create a new instance of the AffixFuzzer15 component."""
        # You can define your own __init__ function as a member of AffixFuzzer15Ext in affix_fuzzer15_ext.py
        self.inner = inner

    # Note: there are no fields here because AffixFuzzer15 delegates to datatypes.AffixFuzzer3


class AffixFuzzer15Type(datatypes.AffixFuzzer3Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer15"


class AffixFuzzer15Batch(datatypes.AffixFuzzer3Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer15Type()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer15Type())
