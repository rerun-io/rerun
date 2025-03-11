# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visualizer_overrides.fbs".

# You can extend this class by creating a "VisualizerOverridesExt" class in "visualizer_overrides_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["VisualizerOverrides"]


@define(str=False, repr=False, init=False)
class VisualizerOverrides(Archetype):
    """
    **Archetype**: Override the visualizers for an entity.

    This archetype is a stop-gap mechanism based on the current implementation details
    of the visualizer system. It is not intended to be a long-term solution, but provides
    enough utility to be useful in the short term.

    The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>

    This can only be used as part of blueprints. It will have no effect if used
    in a regular entity.
    """

    def __init__(self: Any, ranges: datatypes.Utf8ArrayLike) -> None:
        """
        Create a new instance of the VisualizerOverrides archetype.

        Parameters
        ----------
        ranges:
            Names of the visualizers that should be active.

        """

        # You can define your own __init__ function as a member of VisualizerOverridesExt in visualizer_overrides_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(ranges=ranges)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            ranges=None,
        )

    @classmethod
    def _clear(cls) -> VisualizerOverrides:
        """Produce an empty VisualizerOverrides, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        ranges: datatypes.Utf8ArrayLike | None = None,
    ) -> VisualizerOverrides:
        """
        Update only some specific fields of a `VisualizerOverrides`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        ranges:
            Names of the visualizers that should be active.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "ranges": ranges,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> VisualizerOverrides:
        """Clear all the fields of a `VisualizerOverrides`."""
        return cls.from_fields(clear_unset=True)

    ranges: blueprint_components.VisualizerOverrideBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.VisualizerOverrideBatch._converter,  # type: ignore[misc]
    )
    # Names of the visualizers that should be active.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
