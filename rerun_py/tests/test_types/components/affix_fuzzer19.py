# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer19Ext" class in "affix_fuzzer19_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define
from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer19", "AffixFuzzer19Batch", "AffixFuzzer19Type"]


@define(init=False)
class AffixFuzzer19(datatypes.AffixFuzzer5):
    def __init__(self: Any, single_optional_union: datatypes.AffixFuzzer4Like | None = None):
        """Create a new instance of the AffixFuzzer19 component."""

        # You can define your own __init__ function as a member of AffixFuzzer19Ext in affix_fuzzer19_ext.py
        self.__attrs_init__(single_optional_union=single_optional_union)

    # Note: there are no fields here because AffixFuzzer19 delegates to datatypes.AffixFuzzer5


class AffixFuzzer19Type(datatypes.AffixFuzzer5Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer19"


class AffixFuzzer19Batch(datatypes.AffixFuzzer5Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer19Type()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer19Type())
