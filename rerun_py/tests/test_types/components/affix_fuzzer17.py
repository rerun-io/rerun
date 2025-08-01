# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer17Ext" class in "affix_fuzzer17_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
    ComponentMixin,
)

from .. import datatypes

__all__ = ["AffixFuzzer17", "AffixFuzzer17ArrayLike", "AffixFuzzer17Batch", "AffixFuzzer17Like"]


@define(init=False)
class AffixFuzzer17(ComponentMixin):
    _BATCH_TYPE = None

    def __init__(self: Any, many_optional_unions: datatypes.AffixFuzzer3ArrayLike | None = None) -> None:
        """Create a new instance of the AffixFuzzer17 component."""

        # You can define your own __init__ function as a member of AffixFuzzer17Ext in affix_fuzzer17_ext.py
        self.__attrs_init__(many_optional_unions=many_optional_unions)

    many_optional_unions: list[datatypes.AffixFuzzer3] | None = field(default=None)

    def __len__(self) -> int:
        # You can define your own __len__ function as a member of AffixFuzzer17Ext in affix_fuzzer17_ext.py
        return len(self.many_optional_unions) if self.many_optional_unions is not None else 0


AffixFuzzer17Like = AffixFuzzer17
AffixFuzzer17ArrayLike = Union[
    AffixFuzzer17,
    Sequence[AffixFuzzer17Like],
]


class AffixFuzzer17Batch(BaseBatch[AffixFuzzer17ArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.list_(
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
            nullable=True,
            metadata={},
        )
    )
    _COMPONENT_TYPE: str = "rerun.testing.components.AffixFuzzer17"

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer17ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of AffixFuzzer17 not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in affix_fuzzer17_ext.py


# This is patched in late to avoid circular dependencies.
AffixFuzzer17._BATCH_TYPE = AffixFuzzer17Batch  # type: ignore[assignment]
