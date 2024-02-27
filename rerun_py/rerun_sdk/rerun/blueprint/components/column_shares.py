# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/column_shares.fbs".

# You can extend this class by creating a "ColumnSharesExt" class in "column_shares_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from ..._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from ..._converters import (
    to_np_float32,
)

__all__ = ["ColumnShares", "ColumnSharesArrayLike", "ColumnSharesBatch", "ColumnSharesLike", "ColumnSharesType"]


@define(init=False)
class ColumnShares:
    """**Component**: The layout shares of each column in the container."""

    def __init__(self: Any, shares: ColumnSharesLike):
        """
        Create a new instance of the ColumnShares component.

        Parameters
        ----------
        shares:
            The layout shares of each column in the container.

        """

        # You can define your own __init__ function as a member of ColumnSharesExt in column_shares_ext.py
        self.__attrs_init__(shares=shares)

    shares: npt.NDArray[np.float32] = field(converter=to_np_float32)
    # The layout shares of each column in the container.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of ColumnSharesExt in column_shares_ext.py
        return np.asarray(self.shares, dtype=dtype)


ColumnSharesLike = ColumnShares
ColumnSharesArrayLike = Union[
    ColumnShares,
    Sequence[ColumnSharesLike],
]


class ColumnSharesType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.ColumnShares"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={})), self._TYPE_NAME
        )


class ColumnSharesBatch(BaseBatch[ColumnSharesArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ColumnSharesType()

    @staticmethod
    def _native_to_pa_array(data: ColumnSharesArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in column_shares_ext.py
