# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer4Ext" class in "affix_fuzzer4_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer4", "AffixFuzzer4Array", "AffixFuzzer4ArrayLike", "AffixFuzzer4Like", "AffixFuzzer4Type"]


@define
class AffixFuzzer4:
    # You can define your own __init__ function as a member of AffixFuzzer4Ext in affix_fuzzer4_ext.py

    inner: datatypes.AffixFuzzer3 | list[datatypes.AffixFuzzer3] = field()
    """
    single_required (datatypes.AffixFuzzer3):

    many_required (list[datatypes.AffixFuzzer3]):

    many_optional (list[datatypes.AffixFuzzer3]):
    """

    kind: Literal["single_required", "many_required", "many_optional"] = field(default="single_required")


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

# --- Arrow support ---


class AffixFuzzer4Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union(
                [
                    pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                    pa.field(
                        "single_required",
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
                                                        "single_float_optional",
                                                        pa.float32(),
                                                        nullable=True,
                                                        metadata={},
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
                                                        pa.list_(
                                                            pa.field("item", pa.utf8(), nullable=False, metadata={})
                                                        ),
                                                        nullable=False,
                                                        metadata={},
                                                    ),
                                                    pa.field(
                                                        "many_strings_optional",
                                                        pa.list_(
                                                            pa.field("item", pa.utf8(), nullable=False, metadata={})
                                                        ),
                                                        nullable=True,
                                                        metadata={},
                                                    ),
                                                    pa.field(
                                                        "flattened_scalar", pa.float32(), nullable=False, metadata={}
                                                    ),
                                                    pa.field(
                                                        "almost_flattened_scalar",
                                                        pa.struct(
                                                            [
                                                                pa.field(
                                                                    "value", pa.float32(), nullable=False, metadata={}
                                                                )
                                                            ]
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
                    ),
                    pa.field(
                        "many_required",
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
                                                                "single_float_optional",
                                                                pa.float32(),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "single_string_required",
                                                                pa.utf8(),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "single_string_optional",
                                                                pa.utf8(),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_floats_optional",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item",
                                                                        pa.float32(),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    )
                                                                ),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_strings_required",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item", pa.utf8(), nullable=False, metadata={}
                                                                    )
                                                                ),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_strings_optional",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item", pa.utf8(), nullable=False, metadata={}
                                                                    )
                                                                ),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "flattened_scalar",
                                                                pa.float32(),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "almost_flattened_scalar",
                                                                pa.struct(
                                                                    [
                                                                        pa.field(
                                                                            "value",
                                                                            pa.float32(),
                                                                            nullable=False,
                                                                            metadata={},
                                                                        )
                                                                    ]
                                                                ),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "from_parent", pa.bool_(), nullable=True, metadata={}
                                                            ),
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
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "many_optional",
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
                                                                "single_float_optional",
                                                                pa.float32(),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "single_string_required",
                                                                pa.utf8(),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "single_string_optional",
                                                                pa.utf8(),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_floats_optional",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item",
                                                                        pa.float32(),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    )
                                                                ),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_strings_required",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item", pa.utf8(), nullable=False, metadata={}
                                                                    )
                                                                ),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "many_strings_optional",
                                                                pa.list_(
                                                                    pa.field(
                                                                        "item", pa.utf8(), nullable=False, metadata={}
                                                                    )
                                                                ),
                                                                nullable=True,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "flattened_scalar",
                                                                pa.float32(),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "almost_flattened_scalar",
                                                                pa.struct(
                                                                    [
                                                                        pa.field(
                                                                            "value",
                                                                            pa.float32(),
                                                                            nullable=False,
                                                                            metadata={},
                                                                        )
                                                                    ]
                                                                ),
                                                                nullable=False,
                                                                metadata={},
                                                            ),
                                                            pa.field(
                                                                "from_parent", pa.bool_(), nullable=True, metadata={}
                                                            ),
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
                        nullable=False,
                        metadata={},
                    ),
                ]
            ),
            "rerun.testing.datatypes.AffixFuzzer4",
        )


class AffixFuzzer4Array(BaseExtensionArray[AffixFuzzer4ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer4"
    _EXTENSION_TYPE = AffixFuzzer4Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer4ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer4_ext.py


AffixFuzzer4Type._ARRAY_TYPE = AffixFuzzer4Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer4Type())
