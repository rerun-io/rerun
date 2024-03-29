# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/archetypes/background_3d.fbs".

# You can extend this class by creating a "Background3DExt" class in "background3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import components, datatypes
from ..._baseclasses import Archetype
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["Background3D"]


@define(str=False, repr=False, init=False)
class Background3D(Archetype):
    """**Archetype**: Configuration for the background of the 3D space view."""

    def __init__(
        self: Any, kind: blueprint_components.Background3DKindLike, *, color: datatypes.Rgba32Like | None = None
    ):
        """
        Create a new instance of the Background3D archetype.

        Parameters
        ----------
        kind:
            The type of the background. Defaults to DirectionalGradient
        color:
            Color used for Background3DKind.SolidColor.

            Defaults to White.

        """

        # You can define your own __init__ function as a member of Background3DExt in background3d_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(kind=kind, color=color)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            kind=None,  # type: ignore[arg-type]
            color=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Background3D:
        """Produce an empty Background3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    kind: blueprint_components.Background3DKindBatch = field(
        metadata={"component": "required"},
        converter=blueprint_components.Background3DKindBatch._required,  # type: ignore[misc]
    )
    # The type of the background. Defaults to DirectionalGradient
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Color used for Background3DKind.SolidColor.
    #
    # Defaults to White.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
