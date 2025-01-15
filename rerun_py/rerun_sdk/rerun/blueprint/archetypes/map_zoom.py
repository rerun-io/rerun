# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/map_zoom.fbs".

# You can extend this class by creating a "MapZoomExt" class in "map_zoom_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["MapZoom"]


@define(str=False, repr=False, init=False)
class MapZoom(Archetype):
    """**Archetype**: Configuration of the map view zoom level."""

    def __init__(self: Any, zoom: datatypes.Float64Like):
        """
        Create a new instance of the MapZoom archetype.

        Parameters
        ----------
        zoom:
            Zoom level for the map.

            Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).

        """

        # You can define your own __init__ function as a member of MapZoomExt in map_zoom_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(zoom=zoom)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            zoom=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> MapZoom:
        """Produce an empty MapZoom, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        zoom: datatypes.Float64Like | None = None,
    ) -> MapZoom:
        """
        Update only some specific fields of a `MapZoom`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        zoom:
            Zoom level for the map.

            Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).

        """

        kwargs = {
            "zoom": zoom,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return MapZoom(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> MapZoom:
        """Clear all the fields of a `MapZoom`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            zoom=[],  # type: ignore[arg-type]
        )
        return inst

    zoom: blueprint_components.ZoomLevelBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ZoomLevelBatch._optional,  # type: ignore[misc]
    )
    # Zoom level for the map.
    #
    # Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
