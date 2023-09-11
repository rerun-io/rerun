# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".


from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    float_or_none,
)

__all__ = ["AffixFuzzer8", "AffixFuzzer8Array", "AffixFuzzer8ArrayLike", "AffixFuzzer8Like", "AffixFuzzer8Type"]


@define
class AffixFuzzer8:
    single_float_optional: float | None = field(default=None, converter=float_or_none)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.single_float_optional, dtype=dtype)


AffixFuzzer8Like = AffixFuzzer8
AffixFuzzer8ArrayLike = Union[
    AffixFuzzer8,
    Sequence[AffixFuzzer8Like],
]


# --- Arrow support ---


class AffixFuzzer8Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.testing.components.AffixFuzzer8")


class AffixFuzzer8Array(BaseExtensionArray[AffixFuzzer8ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer8"
    _EXTENSION_TYPE = AffixFuzzer8Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer8ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "affixfuzzer8_native_to_pa_array" in rerun_py/rerun_sdk/rerun/_rerun2/components/_overrides/affix_fuzzer8.py


AffixFuzzer8Type._ARRAY_TYPE = AffixFuzzer8Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer8Type())
