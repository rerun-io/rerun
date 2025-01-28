# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visual_bounds2d.fbs".

# You can extend this class by creating a "VisualBounds2DExt" class in "visual_bounds2d_ext.py".

from __future__ import annotations

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions
from .visual_bounds2d_ext import VisualBounds2DExt

__all__ = ["VisualBounds2D"]


@define(str=False, repr=False, init=False)
class VisualBounds2D(VisualBounds2DExt, Archetype):
    """
    **Archetype**: Controls the visual bounds of a 2D view.

    Everything within these bounds are guaranteed to be visible.
    Somethings outside of these bounds may also be visible due to letterboxing.

    If no visual bounds are set, it will be determined automatically,
    based on the bounding-box of the data or other camera information present in the view.
    """

    # __init__ can be found in visual_bounds2d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            range=None,
        )

    @classmethod
    def _clear(cls) -> VisualBounds2D:
        """Produce an empty VisualBounds2D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        range: datatypes.Range2DLike | None = None,
    ) -> VisualBounds2D:
        """
        Update only some specific fields of a `VisualBounds2D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        range:
            Controls the visible range of a 2D view.

            Use this to control pan & zoom of the view.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "range": range,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> VisualBounds2D:
        """Clear all the fields of a `VisualBounds2D`."""
        return cls.from_fields(clear_unset=True)

    range: blueprint_components.VisualBounds2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.VisualBounds2DBatch._converter,  # type: ignore[misc]
    )
    # Controls the visible range of a 2D view.
    #
    # Use this to control pan & zoom of the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
