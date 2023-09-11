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

__all__ = [
    "FlattenedScalar",
    "FlattenedScalarArray",
    "FlattenedScalarArrayLike",
    "FlattenedScalarLike",
    "FlattenedScalarType",
]


@define
class FlattenedScalar:
    # You can define your own __init__ function by defining a function called {init_override_name:?}

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_flattened_scalar_as_array_override"
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


FlattenedScalarLike = FlattenedScalar
FlattenedScalarArrayLike = Union[
    FlattenedScalar,
    Sequence[FlattenedScalarLike],
]


# --- Arrow support ---


class FlattenedScalarType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct([pa.field("value", pa.float32(), nullable=False, metadata={})]),
            "rerun.testing.datatypes.FlattenedScalar",
        )


class FlattenedScalarArray(BaseExtensionArray[FlattenedScalarArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.FlattenedScalar"
    _EXTENSION_TYPE = FlattenedScalarType

    @staticmethod
    def _native_to_pa_array(data: FlattenedScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_flattened_scalar_native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/flattened_scalar.py


FlattenedScalarType._ARRAY_TYPE = FlattenedScalarArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(FlattenedScalarType())
