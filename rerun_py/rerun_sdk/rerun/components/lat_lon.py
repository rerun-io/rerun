# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/latlon.fbs".

# You can extend this class by creating a "LatLonExt" class in "lat_lon_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["LatLon", "LatLonBatch"]


class LatLon(datatypes.DVec2D, ComponentMixin):
    """**Component**: A geospatial position expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees)."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of LatLonExt in lat_lon_ext.py

    # Note: there are no fields here because LatLon delegates to datatypes.DVec2D


class LatLonBatch(datatypes.DVec2DBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.LatLon")


# This is patched in late to avoid circular dependencies.
LatLon._BATCH_TYPE = LatLonBatch  # type: ignore[assignment]
