# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_link.fbs".

# You can extend this class by creating a "ForceLinkExt" class in "force_link_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["ForceLink"]


@define(str=False, repr=False, init=False)
class ForceLink(Archetype):
    """**Archetype**: The link force pushes linked nodes together or apart according to a desired distance."""

    def __init__(
        self: Any,
        *,
        enabled: datatypes.BoolLike | None = None,
        distance: datatypes.Float64Like | None = None,
        iterations: datatypes.UInt64Like | None = None,
    ):
        """
        Create a new instance of the ForceLink archetype.

        Parameters
        ----------
        enabled:
            Whether the force is enabled.
        distance:
            The target distance between two nodes.
        iterations:
            The number of iterations to run the force.

        """

        # You can define your own __init__ function as a member of ForceLinkExt in force_link_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(enabled=enabled, distance=distance, iterations=iterations)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            enabled=None,  # type: ignore[arg-type]
            distance=None,  # type: ignore[arg-type]
            iterations=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ForceLink:
        """Produce an empty ForceLink, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    enabled: blueprint_components.EnabledBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.EnabledBatch._optional,  # type: ignore[misc]
    )
    # Whether the force is enabled.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    distance: blueprint_components.ForceDistanceBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ForceDistanceBatch._optional,  # type: ignore[misc]
    )
    # The target distance between two nodes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    iterations: blueprint_components.ForceIterationsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ForceIterationsBatch._optional,  # type: ignore[misc]
    )
    # The number of iterations to run the force.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
