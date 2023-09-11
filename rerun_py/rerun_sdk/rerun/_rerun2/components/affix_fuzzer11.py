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
    to_np_float32,
)

__all__ = ["AffixFuzzer11", "AffixFuzzer11Array", "AffixFuzzer11ArrayLike", "AffixFuzzer11Like", "AffixFuzzer11Type"]


@define
class AffixFuzzer11:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    many_floats_optional: npt.NDArray[np.float32] | None = field(default=None, converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "affix_fuzzer11__as_array_override"
        return np.asarray(self.many_floats_optional, dtype=dtype)


AffixFuzzer11Like = AffixFuzzer11
AffixFuzzer11ArrayLike = Union[
    AffixFuzzer11,
    Sequence[AffixFuzzer11Like],
]


# --- Arrow support ---


class AffixFuzzer11Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(pa.field("item", pa.float32(), nullable=True, metadata={})),
            "rerun.testing.components.AffixFuzzer11",
        )


class AffixFuzzer11Array(BaseExtensionArray[AffixFuzzer11ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer11"
    _EXTENSION_TYPE = AffixFuzzer11Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer11ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "affix_fuzzer11__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/components/_overrides/affix_fuzzer11.py


AffixFuzzer11Type._ARRAY_TYPE = AffixFuzzer11Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer11Type())
