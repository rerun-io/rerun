# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".


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

__all__ = ["AffixFuzzer2", "AffixFuzzer2Array", "AffixFuzzer2ArrayLike", "AffixFuzzer2Like", "AffixFuzzer2Type"]


@define
class AffixFuzzer2:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    single_float_optional: float | None = field(default=None, converter=float_or_none)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_affix_fuzzer2_as_array"
        return np.asarray(self.single_float_optional, dtype=dtype)


AffixFuzzer2Like = AffixFuzzer2
AffixFuzzer2ArrayLike = Union[
    AffixFuzzer2,
    Sequence[AffixFuzzer2Like],
]


# --- Arrow support ---


class AffixFuzzer2Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.testing.datatypes.AffixFuzzer2")


class AffixFuzzer2Array(BaseExtensionArray[AffixFuzzer2ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer2"
    _EXTENSION_TYPE = AffixFuzzer2Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer2ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_affix_fuzzer2_native_to_pa_array" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/affix_fuzzer2.py


AffixFuzzer2Type._ARRAY_TYPE = AffixFuzzer2Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer2Type())
