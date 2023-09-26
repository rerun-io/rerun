# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer1Ext" class in "affix_fuzzer1_ext.py".

from __future__ import annotations

from typing import Any

import numpy.typing as npt
from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer1", "AffixFuzzer1Batch", "AffixFuzzer1Type"]


class AffixFuzzer1(datatypes.AffixFuzzer1):
    def __init__(
        self: Any,
        single_string_required: str,
        many_strings_required: list[str],
        flattened_scalar: float,
        almost_flattened_scalar: datatypes.FlattenedScalarLike,
        single_float_optional: float | None = None,
        single_string_optional: str | None = None,
        many_floats_optional: npt.ArrayLike | None = None,
        many_strings_optional: list[str] | None = None,
        from_parent: bool | None = None,
    ):
        # You can define your own __init__ function as a member of AffixFuzzer1Ext in affix_fuzzer1_ext.py
        self.__attrs_init__(
            single_float_optional=single_float_optional,
            single_string_required=single_string_required,
            single_string_optional=single_string_optional,
            many_floats_optional=many_floats_optional,
            many_strings_required=many_strings_required,
            many_strings_optional=many_strings_optional,
            flattened_scalar=flattened_scalar,
            almost_flattened_scalar=almost_flattened_scalar,
            from_parent=from_parent,
        )

    # Note: there are no fields here because AffixFuzzer1 delegates to datatypes.AffixFuzzer1


class AffixFuzzer1Type(datatypes.AffixFuzzer1Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer1"


class AffixFuzzer1Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer1Type()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer1Type())
