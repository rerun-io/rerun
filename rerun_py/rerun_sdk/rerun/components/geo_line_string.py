# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/geo_line_string.fbs".

# You can extend this class by creating a "GeoLineStringExt" class in "geo_line_string_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)
from .geo_line_string_ext import GeoLineStringExt

__all__ = ["GeoLineString", "GeoLineStringArrayLike", "GeoLineStringBatch", "GeoLineStringLike", "GeoLineStringType"]


@define(init=False)
class GeoLineString(GeoLineStringExt, ComponentMixin):
    """**Component**: A geospatial line string expressed in EPSG:4326 latitude and longitude."""

    _BATCH_TYPE = None

    def __init__(self: Any, lat_lon: GeoLineStringLike):
        """Create a new instance of the GeoLineString component."""

        # You can define your own __init__ function as a member of GeoLineStringExt in geo_line_string_ext.py
        self.__attrs_init__(lat_lon=lat_lon)

    lat_lon: list[datatypes.DVec2D] = field()


if TYPE_CHECKING:
    GeoLineStringLike = Union[GeoLineString, datatypes.DVec2DArrayLike, npt.NDArray[np.float64]]
else:
    GeoLineStringLike = Any

GeoLineStringArrayLike = Union[GeoLineString, Sequence[GeoLineStringLike], npt.NDArray[np.float64]]


class GeoLineStringType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.GeoLineString"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.list_(pa.field("item", pa.float64(), nullable=False, metadata={}), 2),
                    nullable=False,
                    metadata={},
                )
            ),
            self._TYPE_NAME,
        )


class GeoLineStringBatch(BaseBatch[GeoLineStringArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = GeoLineStringType()

    @staticmethod
    def _native_to_pa_array(data: GeoLineStringArrayLike, data_type: pa.DataType) -> pa.Array:
        return GeoLineStringExt.native_to_pa_array_override(data, data_type)


# This is patched in late to avoid circular dependencies.
GeoLineString._BATCH_TYPE = GeoLineStringBatch  # type: ignore[assignment]
