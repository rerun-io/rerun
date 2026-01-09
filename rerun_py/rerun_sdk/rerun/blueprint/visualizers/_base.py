"""Base class for visualizer configuration."""

from __future__ import annotations

from typing import Any, Protocol, runtime_checkable

from ..._baseclasses import DescribedComponentBatch


class Visualizer:
    """
    Class for visualizer configuration.

    Visualizers can now be created directly from archetype classes.
    This class wraps an archetype instance for use in view overrides.
    """

    def __init__(
        self, visualizer_type: str, *, overrides: list[DescribedComponentBatch] = None, mappings: Any = None
    ) -> None:
        """
        Create a visualizer from an archetype instance.

        Parameters
        ----------
        visualizable_archetype:
            An archetype instance (e.g., rr.Image.from_fields(opacity=0.5))
        mappings:
            Optional component name mappings (currently unused)

        """
        self.visualizer_type = visualizer_type
        self.overrides = overrides
        self.mappings = mappings or []


@runtime_checkable
class VisualizableArchetype(Protocol):
    """Protocol for archetypes that can be visualized."""

    def visualizer(self) -> Visualizer:
        """Creates a visualizer from this archetype."""
        ...
