# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/archetypes/axes3d.fbs".

# You can extend this class by creating a "Axes3DExt" class in "axes3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype
from ..error_utils import catch_and_log_exceptions

__all__ = ["Axes3D"]


@define(str=False, repr=False, init=False)
class Axes3D(Archetype):
    """
    **Archetype**: This archetype shows a set of orthogonal coordinate axes such as for reprsenting a transform.

    See [`Transform3D`][rerun.archetypes.Transform3D]
    """

    def __init__(self: Any, *, length: datatypes.Float32Like | None = None):
        """
        Create a new instance of the Axes3D archetype.

        Parameters
        ----------
        length:
            Length of the 3 axes.

        """

        # You can define your own __init__ function as a member of Axes3DExt in axes3d_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(length=length)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            length=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Axes3D:
        """Produce an empty Axes3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    length: components.AxisLengthBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AxisLengthBatch._optional,  # type: ignore[misc]
    )
    # Length of the 3 axes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
