# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/series_point.fbs".

# You can extend this class by creating a "SeriesPointExt" class in "series_point_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype
from ..error_utils import catch_and_log_exceptions

__all__ = ["SeriesPoint"]


@define(str=False, repr=False, init=False)
class SeriesPoint(Archetype):
    """**Archetype**: Define the style properties for a point series in a chart."""

    def __init__(
        self: Any,
        *,
        color: datatypes.Rgba32Like | None = None,
        marker: components.MarkerShapeLike | None = None,
        name: datatypes.Utf8Like | None = None,
        marker_size: components.MarkerSizeLike | None = None,
    ):
        """
        Create a new instance of the SeriesPoint archetype.

        Parameters
        ----------
        color:
            Color for the corresponding series.
        marker:
            What shape to use to represent the point
        name:
            Display name of the series.

            Used in the legend.
        marker_size:
            Size of the marker.
        """

        # You can define your own __init__ function as a member of SeriesPointExt in series_point_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(color=color, marker=marker, name=name, marker_size=marker_size)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            color=None,  # type: ignore[arg-type]
            marker=None,  # type: ignore[arg-type]
            name=None,  # type: ignore[arg-type]
            marker_size=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> SeriesPoint:
        """Produce an empty SeriesPoint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Color for the corresponding series.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    marker: components.MarkerShapeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MarkerShapeBatch._optional,  # type: ignore[misc]
    )
    # What shape to use to represent the point
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    name: components.NameBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.NameBatch._optional,  # type: ignore[misc]
    )
    # Display name of the series.
    #
    # Used in the legend.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    marker_size: components.MarkerSizeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MarkerSizeBatch._optional,  # type: ignore[misc]
    )
    # Size of the marker.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
