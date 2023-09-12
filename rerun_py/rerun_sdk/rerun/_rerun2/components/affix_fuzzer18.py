# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".


from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer18", "AffixFuzzer18Array", "AffixFuzzer18ArrayLike", "AffixFuzzer18Like", "AffixFuzzer18Type"]


@define
class AffixFuzzer18:
    # You can define your own __init__ function by defining a function called "affix_fuzzer18__init_override"

    many_optional_unions: list[datatypes.AffixFuzzer4] | None = field(default=None)


AffixFuzzer18Like = AffixFuzzer18
AffixFuzzer18ArrayLike = Union[
    AffixFuzzer18,
    Sequence[AffixFuzzer18Like],
]


# --- Arrow support ---


class AffixFuzzer18Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
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
                                                                        "item", pa.float32(), nullable=True, metadata={}
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
                                                                        "item", pa.utf8(), nullable=True, metadata={}
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
                                                                                nullable=True,
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
                                                                                "item",
                                                                                pa.utf8(),
                                                                                nullable=False,
                                                                                metadata={},
                                                                            )
                                                                        ),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_optional",
                                                                        pa.list_(
                                                                            pa.field(
                                                                                "item",
                                                                                pa.utf8(),
                                                                                nullable=True,
                                                                                metadata={},
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
                                                                        "from_parent",
                                                                        pa.bool_(),
                                                                        nullable=True,
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
                                                    "fixed_size_shenanigans",
                                                    pa.list_(
                                                        pa.field("item", pa.float32(), nullable=False, metadata={}), 3
                                                    ),
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
                                                                                nullable=True,
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
                                                                                "item",
                                                                                pa.utf8(),
                                                                                nullable=False,
                                                                                metadata={},
                                                                            )
                                                                        ),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_optional",
                                                                        pa.list_(
                                                                            pa.field(
                                                                                "item",
                                                                                pa.utf8(),
                                                                                nullable=True,
                                                                                metadata={},
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
                                                                        "from_parent",
                                                                        pa.bool_(),
                                                                        nullable=True,
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
                                                    "fixed_size_shenanigans",
                                                    pa.list_(
                                                        pa.field("item", pa.float32(), nullable=False, metadata={}), 3
                                                    ),
                                                    nullable=False,
                                                    metadata={},
                                                ),
                                            ]
                                        ),
                                        nullable=True,
                                        metadata={},
                                    )
                                ),
                                nullable=False,
                                metadata={},
                            ),
                        ]
                    ),
                    nullable=True,
                    metadata={},
                )
            ),
            "rerun.testing.components.AffixFuzzer18",
        )


class AffixFuzzer18Array(BaseExtensionArray[AffixFuzzer18ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer18"
    _EXTENSION_TYPE = AffixFuzzer18Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer18ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "affix_fuzzer18__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/components/_overrides/affix_fuzzer18.py


AffixFuzzer18Type._ARRAY_TYPE = AffixFuzzer18Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer18Type())
