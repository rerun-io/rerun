# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".


from __future__ import annotations

from typing import Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    bool_or_none,
    float_or_none,
    str_or_none,
    to_np_float32,
)

__all__ = ["AffixFuzzer1", "AffixFuzzer1Array", "AffixFuzzer1ArrayLike", "AffixFuzzer1Like", "AffixFuzzer1Type"]


def _override_affix_fuzzer1_almost_flattened_scalar_converter(
    x: datatypes.FlattenedScalarLike,
) -> datatypes.FlattenedScalar:
    if isinstance(x, datatypes.FlattenedScalar):
        return x
    else:
        return datatypes.FlattenedScalar(x)


@define
class AffixFuzzer1:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    single_string_required: str = field(converter=str)
    many_strings_required: list[str] = field()
    flattened_scalar: float = field(converter=float)
    almost_flattened_scalar: datatypes.FlattenedScalar = field(
        converter=_override_affix_fuzzer1_almost_flattened_scalar_converter
    )
    single_float_optional: float | None = field(default=None, converter=float_or_none)
    single_string_optional: str | None = field(default=None, converter=str_or_none)
    many_floats_optional: npt.NDArray[np.float32] | None = field(default=None, converter=to_np_float32)
    many_strings_optional: list[str] | None = field(default=None)
    from_parent: bool | None = field(default=None, converter=bool_or_none)


AffixFuzzer1Like = AffixFuzzer1
AffixFuzzer1ArrayLike = Union[
    AffixFuzzer1,
    Sequence[AffixFuzzer1Like],
]


# --- Arrow support ---


class AffixFuzzer1Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
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
            "rerun.testing.datatypes.AffixFuzzer1",
        )


class AffixFuzzer1Array(BaseExtensionArray[AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer1"
    _EXTENSION_TYPE = AffixFuzzer1Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer1ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_affix_fuzzer1_native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/affix_fuzzer1.py


AffixFuzzer1Type._ARRAY_TYPE = AffixFuzzer1Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer1Type())
