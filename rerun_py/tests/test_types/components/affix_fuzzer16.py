# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer16Ext" class in "affix_fuzzer16_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer16", "AffixFuzzer16ArrayLike", "AffixFuzzer16Batch", "AffixFuzzer16Like", "AffixFuzzer16Type"]


@define(init=False)
class AffixFuzzer16(ComponentMixin):
    _BATCH_TYPE = None

    def __init__(self: Any, many_required_unions: AffixFuzzer16Like):
        """Create a new instance of the AffixFuzzer16 component."""

        # You can define your own __init__ function as a member of AffixFuzzer16Ext in affix_fuzzer16_ext.py
        self.__attrs_init__(many_required_unions=many_required_unions)

    many_required_unions: list[datatypes.AffixFuzzer3] = field()


AffixFuzzer16Like = AffixFuzzer16
AffixFuzzer16ArrayLike = Union[
    AffixFuzzer16,
    Sequence[AffixFuzzer16Like],
]


class AffixFuzzer16Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer16"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.dense_union([
                        pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                        pa.field("degrees", pa.float32(), nullable=False, metadata={}),
                        pa.field(
                            "craziness",
                            pa.list_(
                                pa.field(
                                    "item",
                                    pa.struct([
                                        pa.field("single_float_optional", pa.float32(), nullable=True, metadata={}),
                                        pa.field("single_string_required", pa.utf8(), nullable=False, metadata={}),
                                        pa.field("single_string_optional", pa.utf8(), nullable=True, metadata={}),
                                        pa.field(
                                            "many_floats_optional",
                                            pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={})),
                                            nullable=True,
                                            metadata={},
                                        ),
                                        pa.field(
                                            "many_strings_required",
                                            pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={})),
                                            nullable=False,
                                            metadata={},
                                        ),
                                        pa.field(
                                            "many_strings_optional",
                                            pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={})),
                                            nullable=True,
                                            metadata={},
                                        ),
                                        pa.field("flattened_scalar", pa.float32(), nullable=False, metadata={}),
                                        pa.field(
                                            "almost_flattened_scalar",
                                            pa.struct([pa.field("value", pa.float32(), nullable=False, metadata={})]),
                                            nullable=False,
                                            metadata={},
                                        ),
                                        pa.field("from_parent", pa.bool_(), nullable=True, metadata={}),
                                    ]),
                                    nullable=False,
                                    metadata={},
                                )
                            ),
                            nullable=False,
                            metadata={},
                        ),
                        pa.field(
                            "fixed_size_shenanigans",
                            pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                            nullable=False,
                            metadata={},
                        ),
                        pa.field("empty_variant", pa.null(), nullable=True, metadata={}),
                    ]),
                    nullable=False,
                    metadata={},
                )
            ),
            self._TYPE_NAME,
        )


class AffixFuzzer16Batch(BaseBatch[AffixFuzzer16ArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer16Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer16ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of AffixFuzzer16 not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in affix_fuzzer16_ext.py


# This is patched in late to avoid circular dependencies.
AffixFuzzer16._BATCH_TYPE = AffixFuzzer16Batch  # type: ignore[assignment]
