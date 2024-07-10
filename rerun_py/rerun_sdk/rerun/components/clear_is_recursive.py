# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/clear_is_recursive.fbs".

# You can extend this class by creating a "ClearIsRecursiveExt" class in "clear_is_recursive_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = [
    "ClearIsRecursive",
    "ClearIsRecursiveArrayLike",
    "ClearIsRecursiveBatch",
    "ClearIsRecursiveLike",
    "ClearIsRecursiveType",
]


@define(init=False)
class ClearIsRecursive(ComponentMixin):
    """**Component**: Configures how a clear operation should behave - recursive or not."""

    _BATCH_TYPE = None

    def __init__(self: Any, recursive: ClearIsRecursiveLike):
        """
        Create a new instance of the ClearIsRecursive component.

        Parameters
        ----------
        recursive:
            If true, also clears all recursive children entities.

        """

        # You can define your own __init__ function as a member of ClearIsRecursiveExt in clear_is_recursive_ext.py
        self.__attrs_init__(recursive=recursive)

    def __bool__(self) -> bool:
        return self.recursive

    recursive: bool = field(converter=bool)
    # If true, also clears all recursive children entities.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


if TYPE_CHECKING:
    ClearIsRecursiveLike = Union[ClearIsRecursive, bool]
else:
    ClearIsRecursiveLike = Any

ClearIsRecursiveArrayLike = Union[ClearIsRecursive, Sequence[ClearIsRecursiveLike], bool, npt.NDArray[np.bool_]]


class ClearIsRecursiveType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.ClearIsRecursive"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), self._TYPE_NAME)


class ClearIsRecursiveBatch(BaseBatch[ClearIsRecursiveArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ClearIsRecursiveType()

    @staticmethod
    def _native_to_pa_array(data: ClearIsRecursiveArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.bool_).flatten()
        return pa.array(array, type=data_type)


# This is patched in late to avoid circular dependencies.
ClearIsRecursive._BATCH_TYPE = ClearIsRecursiveBatch  # type: ignore[assignment]
