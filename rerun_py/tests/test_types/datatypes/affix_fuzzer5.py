# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer5Ext" class in "affix_fuzzer5_ext.py".

from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType

from .. import datatypes

__all__ = ["AffixFuzzer5", "AffixFuzzer5ArrayLike", "AffixFuzzer5Batch", "AffixFuzzer5Like", "AffixFuzzer5Type"]


def _affix_fuzzer5__single_optional_union__special_field_converter_override(
    x: datatypes.AffixFuzzer4Like | None,
) -> datatypes.AffixFuzzer4 | None:
    if x is None:
        return None
    elif isinstance(x, datatypes.AffixFuzzer4):
        return x
    else:
        return datatypes.AffixFuzzer4(x)


@define
class AffixFuzzer5:
    # You can define your own __init__ function as a member of AffixFuzzer5Ext in affix_fuzzer5_ext.py

    single_optional_union: datatypes.AffixFuzzer4 | None = field(
        default=None, converter=_affix_fuzzer5__single_optional_union__special_field_converter_override
    )


AffixFuzzer5Like = AffixFuzzer5
AffixFuzzer5ArrayLike = Union[
    AffixFuzzer5,
    Sequence[AffixFuzzer5Like],
]


class AffixFuzzer5Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.AffixFuzzer5"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field(
                        "single_optional_union",
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
                                                                            nullable=False,
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
                                                                                    nullable=False,
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
                                                            pa.field("item", pa.float32(), nullable=False, metadata={}),
                                                            3,
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
                                                                                    nullable=False,
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
                                                            pa.field("item", pa.float32(), nullable=False, metadata={}),
                                                            3,
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
                            ]
                        ),
                        nullable=True,
                        metadata={},
                    )
                ]
            ),
            self._TYPE_NAME,
        )


class AffixFuzzer5Batch(BaseBatch[AffixFuzzer5ArrayLike]):
    _ARROW_TYPE = AffixFuzzer5Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer5ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer5_ext.py


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer5Type())
