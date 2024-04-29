# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer4Ext" class in "affix_fuzzer4_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType

from .. import datatypes

__all__ = ["AffixFuzzer4", "AffixFuzzer4ArrayLike", "AffixFuzzer4Batch", "AffixFuzzer4Like", "AffixFuzzer4Type"]


@define
class AffixFuzzer4:
    # You can define your own __init__ function as a member of AffixFuzzer4Ext in affix_fuzzer4_ext.py

    inner: Union[datatypes.AffixFuzzer3, list[datatypes.AffixFuzzer3]] = field()
    """
    Must be one of:

    * single_required (datatypes.AffixFuzzer3):

    * many_required (list[datatypes.AffixFuzzer3]):

    * many_optional (list[datatypes.AffixFuzzer3]):
    """

    kind: Literal["single_required", "many_required", "many_optional"] = field(default="single_required")
    """
    Possible values:

    * "single_required":

    * "many_required":

    * "many_optional":
    """


if TYPE_CHECKING:
    AffixFuzzer4Like = Union[
        AffixFuzzer4,
        datatypes.AffixFuzzer3,
        list[datatypes.AffixFuzzer3],
    ]
    AffixFuzzer4ArrayLike = Union[
        AffixFuzzer4,
        datatypes.AffixFuzzer3,
        list[datatypes.AffixFuzzer3],
        Sequence[AffixFuzzer4Like],
    ]
else:
    AffixFuzzer4Like = Any
    AffixFuzzer4ArrayLike = Any


class AffixFuzzer4Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.AffixFuzzer4"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field(
                    "single_required",
                    pa.dense_union([
                        pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                        pa.field("degrees", pa.float32(), nullable=False, metadata={}),
                        pa.field("radians", pa.float32(), nullable=False, metadata={}),
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
                    ]),
                    nullable=False,
                    metadata={},
                ),
                pa.field(
                    "many_required",
                    pa.list_(
                        pa.field(
                            "item",
                            pa.dense_union([
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field("degrees", pa.float32(), nullable=False, metadata={}),
                                pa.field("radians", pa.float32(), nullable=False, metadata={}),
                                pa.field(
                                    "craziness",
                                    pa.list_(
                                        pa.field(
                                            "item",
                                            pa.struct([
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
                                                    pa.struct([
                                                        pa.field("value", pa.float32(), nullable=False, metadata={})
                                                    ]),
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
                            ]),
                            nullable=False,
                            metadata={},
                        )
                    ),
                    nullable=False,
                    metadata={},
                ),
                pa.field(
                    "many_optional",
                    pa.list_(
                        pa.field(
                            "item",
                            pa.dense_union([
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field("degrees", pa.float32(), nullable=False, metadata={}),
                                pa.field("radians", pa.float32(), nullable=False, metadata={}),
                                pa.field(
                                    "craziness",
                                    pa.list_(
                                        pa.field(
                                            "item",
                                            pa.struct([
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
                                                    pa.struct([
                                                        pa.field("value", pa.float32(), nullable=False, metadata={})
                                                    ]),
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
                            ]),
                            nullable=False,
                            metadata={},
                        )
                    ),
                    nullable=False,
                    metadata={},
                ),
            ]),
            self._TYPE_NAME,
        )


class AffixFuzzer4Batch(BaseBatch[AffixFuzzer4ArrayLike]):
    _ARROW_TYPE = AffixFuzzer4Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer4ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of AffixFuzzer4 not implemented: We lack codegen for arrow-serialization of unions"
        )  # You need to implement native_to_pa_array_override in affix_fuzzer4_ext.py
