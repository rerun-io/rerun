# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer22Ext" class in "affix_fuzzer22_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType
from rerun._converters import (
    to_np_uint8,
)

__all__ = ["AffixFuzzer22", "AffixFuzzer22ArrayLike", "AffixFuzzer22Batch", "AffixFuzzer22Like", "AffixFuzzer22Type"]


@define(init=False)
class AffixFuzzer22:
    def __init__(self: Any, fixed_sized_native: AffixFuzzer22Like):
        """Create a new instance of the AffixFuzzer22 datatype."""

        # You can define your own __init__ function as a member of AffixFuzzer22Ext in affix_fuzzer22_ext.py
        self.__attrs_init__(fixed_sized_native=fixed_sized_native)

    fixed_sized_native: npt.NDArray[np.uint8] = field(converter=to_np_uint8)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of AffixFuzzer22Ext in affix_fuzzer22_ext.py
        return np.asarray(self.fixed_sized_native, dtype=dtype)


AffixFuzzer22Like = AffixFuzzer22
AffixFuzzer22ArrayLike = Union[
    AffixFuzzer22,
    Sequence[AffixFuzzer22Like],
]


class AffixFuzzer22Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.AffixFuzzer22"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field(
                        "fixed_sized_native",
                        pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={}), 4),
                        nullable=False,
                        metadata={},
                    )
                ]
            ),
            self._TYPE_NAME,
        )


class AffixFuzzer22Batch(BaseBatch[AffixFuzzer22ArrayLike]):
    _ARROW_TYPE = AffixFuzzer22Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer22ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in affix_fuzzer22_ext.py
