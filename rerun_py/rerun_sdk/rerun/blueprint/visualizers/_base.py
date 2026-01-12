"""Base class for visualizer configuration."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

if TYPE_CHECKING:
    from ..._baseclasses import DescribedComponentBatch


class Visualizer:
    """
    Class for visualizer configuration.

    Visualizers can now be created directly from archetype classes.
    This class wraps an archetype instance for use in view overrides.
    """

    def __init__(
        self, visualizer_type: str, *, overrides: list[DescribedComponentBatch] | None = None, mappings: Any = None
    ) -> None:
        """
        Create a visualizer from an archetype instance.

        Parameters
        ----------
        visualizer_type:
            The type name of the visualizer.
        overrides:
            Any component overrides to apply to fields of the visualizer.
        mappings:
            Optional component name mappings.
            TODO(RR-3254): Currently unused - implement mapping functionality

        """
        self.visualizer_type = visualizer_type
        self.overrides = overrides
        self.mappings = mappings or []


@runtime_checkable
class VisualizableArchetype(Protocol):
    """Protocol for archetypes that can be visualized."""

    # Note that we allow arbitrary args and kwargs so that implementors
    # can extend this method for setting up component mappings.
    def visualizer(self, *args: Any, **kwargs: Any) -> Visualizer:
        """Creates a visualizer from this archetype."""
        ...
