# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".


from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["AffixFuzzer20", "AffixFuzzer20Array", "AffixFuzzer20ArrayLike", "AffixFuzzer20Like", "AffixFuzzer20Type"]


def _affix_fuzzer20__p__special_field_converter_override(
    x: datatypes.PrimitiveComponentLike,
) -> datatypes.PrimitiveComponent:
    if isinstance(x, datatypes.PrimitiveComponent):
        return x
    else:
        return datatypes.PrimitiveComponent(x)


def _affix_fuzzer20__s__special_field_converter_override(x: datatypes.StringComponentLike) -> datatypes.StringComponent:
    if isinstance(x, datatypes.StringComponent):
        return x
    else:
        return datatypes.StringComponent(x)


@define
class AffixFuzzer20:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    p: datatypes.PrimitiveComponent = field(converter=_affix_fuzzer20__p__special_field_converter_override)
    s: datatypes.StringComponent = field(converter=_affix_fuzzer20__s__special_field_converter_override)


AffixFuzzer20Like = AffixFuzzer20
AffixFuzzer20ArrayLike = Union[
    AffixFuzzer20,
    Sequence[AffixFuzzer20Like],
]


# --- Arrow support ---


class AffixFuzzer20Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("p", pa.uint32(), nullable=False, metadata={}),
                    pa.field("s", pa.utf8(), nullable=False, metadata={}),
                ]
            ),
            "rerun.testing.datatypes.AffixFuzzer20",
        )


class AffixFuzzer20Array(BaseExtensionArray[AffixFuzzer20ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer20"
    _EXTENSION_TYPE = AffixFuzzer20Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer20ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "affix_fuzzer20__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/affix_fuzzer20.py


AffixFuzzer20Type._ARRAY_TYPE = AffixFuzzer20Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer20Type())
