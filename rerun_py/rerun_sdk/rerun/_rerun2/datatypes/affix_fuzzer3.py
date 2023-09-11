# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".


from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer3", "AffixFuzzer3Array", "AffixFuzzer3ArrayLike", "AffixFuzzer3Like", "AffixFuzzer3Type"]


@define
class AffixFuzzer3:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    inner: float | list[datatypes.AffixFuzzer1] | npt.NDArray[np.float32] = field()
    """
    degrees (float):

    radians (float):

    craziness (list[datatypes.AffixFuzzer1]):

    fixed_size_shenanigans (npt.NDArray[np.float32]):
    """

    kind: Literal["degrees", "radians", "craziness", "fixed_size_shenanigans"] = field(default="degrees")


if TYPE_CHECKING:
    AffixFuzzer3Like = Union[
        AffixFuzzer3,
        float,
        list[datatypes.AffixFuzzer1],
        npt.NDArray[np.float32],
    ]
    AffixFuzzer3ArrayLike = Union[
        AffixFuzzer3,
        float,
        list[datatypes.AffixFuzzer1],
        npt.NDArray[np.float32],
        Sequence[AffixFuzzer3Like],
    ]
else:
    AffixFuzzer3Like = Any
    AffixFuzzer3ArrayLike = Any

# --- Arrow support ---


class AffixFuzzer3Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
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
                                        pa.field("single_float_optional", pa.float32(), nullable=True, metadata={}),
                                        pa.field("single_string_required", pa.utf8(), nullable=False, metadata={}),
                                        pa.field("single_string_optional", pa.utf8(), nullable=True, metadata={}),
                                        pa.field(
                                            "many_floats_optional",
                                            pa.list_(pa.field("item", pa.float32(), nullable=True, metadata={})),
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
                                            pa.list_(pa.field("item", pa.utf8(), nullable=True, metadata={})),
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
            "rerun.testing.datatypes.AffixFuzzer3",
        )


class AffixFuzzer3Array(BaseExtensionArray[AffixFuzzer3ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer3"
    _EXTENSION_TYPE = AffixFuzzer3Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer3ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "affix_fuzzer3__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/affix_fuzzer3.py


AffixFuzzer3Type._ARRAY_TYPE = AffixFuzzer3Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer3Type())
