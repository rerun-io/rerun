# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer10Ext" class in "affix_fuzzer10_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)
from rerun._converters import (
    str_or_none,
)

__all__ = ["AffixFuzzer10", "AffixFuzzer10ArrayLike", "AffixFuzzer10Batch", "AffixFuzzer10Like", "AffixFuzzer10Type"]


@define(init=False)
class AffixFuzzer10(ComponentMixin):
    _BATCH_TYPE = None

    def __init__(self: Any, single_string_optional: str | None = None):
        """Create a new instance of the AffixFuzzer10 component."""

        # You can define your own __init__ function as a member of AffixFuzzer10Ext in affix_fuzzer10_ext.py
        self.__attrs_init__(single_string_optional=single_string_optional)

    single_string_optional: str | None = field(default=None, converter=str_or_none)


AffixFuzzer10Like = AffixFuzzer10
AffixFuzzer10ArrayLike = Union[
    AffixFuzzer10,
    Sequence[AffixFuzzer10Like],
]


class AffixFuzzer10Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.components.AffixFuzzer10"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), self._TYPE_NAME)


class AffixFuzzer10Batch(BaseBatch[AffixFuzzer10ArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AffixFuzzer10Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer10ArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array: Union[list[str], npt.ArrayLike] = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        elif isinstance(data, np.ndarray):
            array = data
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)


# This is patched in late to avoid circular dependencies.
AffixFuzzer10._BATCH_TYPE = AffixFuzzer10Batch  # type: ignore[assignment]
