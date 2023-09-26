# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer4Ext" class in "affix_fuzzer4_ext.py".

from __future__ import annotations

from typing import Any

import numpy.typing as npt
from rerun._baseclasses import ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer4", "AffixFuzzer4Batch", "AffixFuzzer4Type"]


class AffixFuzzer4(datatypes.AffixFuzzer1):
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
        """Create a new instance of the AffixFuzzer4 component."""

        # You can define your own __init__ function as a member of AffixFuzzer4Ext in affix_fuzzer4_ext.py
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

    # Note: there are no fields here because AffixFuzzer4 delegates to datatypes.AffixFuzzer1


class AffixFuzzer4Type(datatypes.AffixFuzzer1Type):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer4"


class AffixFuzzer4Batch(datatypes.AffixFuzzer1Batch, ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer4Type()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer4Type())
