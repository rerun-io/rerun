# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer17Ext" class in "affix_fuzzer17_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

from .. import datatypes

__all__ = ["AffixFuzzer17", "AffixFuzzer17ArrayLike", "AffixFuzzer17Batch", "AffixFuzzer17Like", "AffixFuzzer17Type"]


@define(init=False)
class AffixFuzzer17:
    def __init__(self: Any, many_optional_unions: datatypes.AffixFuzzer3ArrayLike | None = None):
        """Create a new instance of the AffixFuzzer17 component."""

        # You can define your own __init__ function as a member of AffixFuzzer17Ext in affix_fuzzer17_ext.py
        self.__attrs_init__(many_optional_unions=many_optional_unions)

    many_optional_unions: list[datatypes.AffixFuzzer3] | None = field(default=None)


AffixFuzzer17Like = AffixFuzzer17
AffixFuzzer17ArrayLike = Union[
    AffixFuzzer17,
    Sequence[AffixFuzzer17Like],
]


class AffixFuzzer17Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer17"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.dense_union(
                        [
                            pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                            pa.field("degrees", pa.float32(), nullable=False, metadata={}),
                            pa.field("radians", pa.float32(), nullable=False, metadata={}),
                            pa.field(
                                "craziness",
                                pa.list_(
                                    pa.field(
                                        "item",
                                        pa.struct(
                                            [
                                                pa.field(
                                                    "single_float_optional", pa.float32(), nullable=True, metadata={}
                                                ),
                                                pa.field(
                                                    "single_string_required", pa.utf8(), nullable=False, metadata={}
                                                ),
                                                pa.field(
                                                    "single_string_optional", pa.utf8(), nullable=True, metadata={}
                                                ),
                                                pa.field(
                                                    "many_floats_optional",
                                                    pa.list_(
                                                        pa.field("item", pa.float32(), nullable=False, metadata={})
                                                    ),
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
                                                    pa.struct(
                                                        [pa.field("value", pa.float32(), nullable=False, metadata={})]
                                                    ),
                                                    nullable=False,
                                                    metadata={},
                                                ),
                                                pa.field("from_parent", pa.bool_(), nullable=True, metadata={}),
                                            ]
                                        ),
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
                        ]
                    ),
                    nullable=False,
                    metadata={},
                )
            ),
            self._TYPE_NAME,
        )


class AffixFuzzer17Batch(BaseBatch[AffixFuzzer17ArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer17Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer17ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer17_ext.py
