# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer11Ext" class in "affix_fuzzer11_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)
from rerun._converters import (
    to_np_float32,
)

__all__ = ["AffixFuzzer11", "AffixFuzzer11ArrayLike", "AffixFuzzer11Batch", "AffixFuzzer11Like"]


@define(init=False)
class AffixFuzzer11(ComponentMixin):
    _BATCH_TYPE = None

    def __init__(self: Any, many_floats_optional: npt.ArrayLike | None = None) -> None:
        """Create a new instance of the AffixFuzzer11 component."""

        # You can define your own __init__ function as a member of AffixFuzzer11Ext in affix_fuzzer11_ext.py
        self.__attrs_init__(many_floats_optional=many_floats_optional)

    many_floats_optional: npt.NDArray[np.float32] | None = field(
        default=None,
        converter=to_np_float32,
    )

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of AffixFuzzer11Ext in affix_fuzzer11_ext.py
        return np.asarray(self.many_floats_optional, dtype=dtype, copy=copy)


AffixFuzzer11Like = AffixFuzzer11
AffixFuzzer11ArrayLike = Union[
    AffixFuzzer11,
    Sequence[AffixFuzzer11Like],
]


class AffixFuzzer11Batch(BaseBatch[AffixFuzzer11ArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}))
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.testing.components.AffixFuzzer11")

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer11ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of AffixFuzzer11 not implemented: We lack codegen for arrow-serialization of general structs",
        )  # You need to implement native_to_pa_array_override in affix_fuzzer11_ext.py


# This is patched in late to avoid circular dependencies.
AffixFuzzer11._BATCH_TYPE = AffixFuzzer11Batch  # type: ignore[assignment]
