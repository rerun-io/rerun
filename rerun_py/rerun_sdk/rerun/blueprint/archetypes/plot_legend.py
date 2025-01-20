# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/plot_legend.fbs".

# You can extend this class by creating a "PlotLegendExt" class in "plot_legend_ext.py".

from __future__ import annotations

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions
from .plot_legend_ext import PlotLegendExt

__all__ = ["PlotLegend"]


@define(str=False, repr=False, init=False)
class PlotLegend(PlotLegendExt, Archetype):
    """**Archetype**: Configuration for the legend of a plot."""

    # __init__ can be found in plot_legend_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            corner=None,
            visible=None,
        )

    @classmethod
    def _clear(cls) -> PlotLegend:
        """Produce an empty PlotLegend, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        corner: blueprint_components.Corner2DLike | None = None,
        visible: datatypes.BoolLike | None = None,
    ) -> PlotLegend:
        """
        Update only some specific fields of a `PlotLegend`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        corner:
            To what corner the legend is aligned.

            Defaults to the right bottom corner.
        visible:
            Whether the legend is shown at all.

            True by default.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "corner": corner,
                "visible": visible,
            }

            if clear:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def clear_fields(cls) -> PlotLegend:
        """Clear all the fields of a `PlotLegend`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            corner=[],
            visible=[],
        )
        return inst

    corner: blueprint_components.Corner2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.Corner2DBatch._converter,  # type: ignore[misc]
    )
    # To what corner the legend is aligned.
    #
    # Defaults to the right bottom corner.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    visible: blueprint_components.VisibleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.VisibleBatch._converter,  # type: ignore[misc]
    )
    # Whether the legend is shown at all.
    #
    # True by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
