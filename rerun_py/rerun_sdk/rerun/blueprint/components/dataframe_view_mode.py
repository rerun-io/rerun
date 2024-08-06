# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/dataframe_view_mode.fbs".

# You can extend this class by creating a "DataframeViewModeExt" class in "dataframe_view_mode_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = [
    "DataframeViewMode",
    "DataframeViewModeArrayLike",
    "DataframeViewModeBatch",
    "DataframeViewModeLike",
    "DataframeViewModeType",
]


from enum import Enum


class DataframeViewMode(Enum):
    """**Component**: The kind of table displayed by the dataframe view."""

    LatestAt = 0
    """
    Display the entity values at the current time.

    In this mode, rows are entity instances, and columns are components. The visible time range setting is ignored.
    """

    TimeRange = 1
    """
    Display a temporal table of entity values.

    In this mode, rows are combination of entity path, timestamp, and row id, and columns are components. The
    timestamp shown are determined by each view entity's visible time range setting.
    """

    @classmethod
    def auto(cls, val: str | int | DataframeViewMode) -> DataframeViewMode:
        """Best-effort converter, including a case-insensitive string matcher."""
        if isinstance(val, DataframeViewMode):
            return val
        if isinstance(val, int):
            return cls(val)
        try:
            return cls[val]
        except KeyError:
            val_lower = val.lower()
            for variant in cls:
                if variant.name.lower() == val_lower:
                    return variant
        raise ValueError(f"Cannot convert {val} to {cls.__name__}")

    def __str__(self) -> str:
        """Returns the variant name."""
        return self.name


DataframeViewModeLike = Union[DataframeViewMode, Literal["LatestAt", "TimeRange", "latestat", "timerange"], int]
DataframeViewModeArrayLike = Union[DataframeViewModeLike, Sequence[DataframeViewModeLike]]


class DataframeViewModeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.DataframeViewMode"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class DataframeViewModeBatch(BaseBatch[DataframeViewModeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = DataframeViewModeType()

    @staticmethod
    def _native_to_pa_array(data: DataframeViewModeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (DataframeViewMode, int, str)):
            data = [data]

        pa_data = [DataframeViewMode.auto(v).value if v is not None else None for v in data]

        return pa.array(pa_data, type=data_type)
