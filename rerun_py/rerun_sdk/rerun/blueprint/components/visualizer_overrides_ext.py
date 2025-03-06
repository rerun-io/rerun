from __future__ import annotations

from typing import cast

from ..._baseclasses import DescribedComponentBatch


class VisualizerOverridesExt:
    """Extension for [VisualizerOverrides][rerun.blueprint.components.VisualizerOverrides]."""

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        """
        Convert the VisualizerOverrides component to a list of component batches.

        This facilitates the special role of VisualizerOverrides as a component that
        can be used directly as an override.
        It intentionally has no archetype name or field name.

        TODO(#8129): In the future visualizer overrides should be handled with tagging overrides instead.
        """
        from ...blueprint.components.visualizer_overrides import VisualizerOverrides

        batch = cast(VisualizerOverrides, self)
        return [DescribedComponentBatch(batch, batch.component_descriptor())]
