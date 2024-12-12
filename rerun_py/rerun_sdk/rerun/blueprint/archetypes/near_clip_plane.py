# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/near_clip_plane.fbs".

# You can extend this class by creating a "NearClipPlaneExt" class in "near_clip_plane_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["NearClipPlane"]


@define(str=False, repr=False, init=False)
class NearClipPlane(Archetype):
    """**Archetype**: Controls the distance to the near clip plane in 3D scene units."""

    def __init__(self: Any, near_clip_plane: datatypes.Float32Like):
        """
        Create a new instance of the NearClipPlane archetype.

        Parameters
        ----------
        near_clip_plane:
            Controls the distance to the near clip plane in 3D scene units.

            Content closer than this distance will not be visible.

        """

        # You can define your own __init__ function as a member of NearClipPlaneExt in near_clip_plane_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(near_clip_plane=near_clip_plane)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            near_clip_plane=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> NearClipPlane:
        """Produce an empty NearClipPlane, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    near_clip_plane: blueprint_components.NearClipPlaneBatch = field(
        metadata={"component": "required"},
        converter=blueprint_components.NearClipPlaneBatch._required,  # type: ignore[misc]
    )
    # Controls the distance to the near clip plane in 3D scene units.
    #
    # Content closer than this distance will not be visible.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
