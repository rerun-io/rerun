"""Base class for visualizer configuration."""

from __future__ import annotations

import uuid
from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

if TYPE_CHECKING:
    from ..._baseclasses import DescribedComponentBatch
    from ...blueprint.datatypes import VisualizerComponentMappingLike


class Visualizer:
    """
    Class for visualizer configuration.

    Visualizers can now be created directly from archetype classes.
    This class wraps an archetype instance for use in view overrides.
    """

    def __init__(
        self,
        visualizer_type: str,
        *,
        overrides: list[DescribedComponentBatch] | None = None,
        mappings: list[VisualizerComponentMappingLike] | None = None,
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
            Optional component name mappings to determine how components are sourced.

            ⚠️TODO(#12600): The API for component mappings is still evolving, so this may change in the future.

        """
        self.id = uuid.uuid4()
        self.visualizer_type = visualizer_type
        self.overrides = overrides
        self.mappings = mappings


@runtime_checkable
class VisualizableArchetype(Protocol):
    """Protocol for archetypes that can be visualized."""

    # Note that we allow arbitrary args and kwargs so that implementors
    # can extend this method for setting up component mappings.
    def visualizer(self, *args: Any, **kwargs: Any) -> Visualizer:
        """Creates a visualizer from this archetype."""
        ...
