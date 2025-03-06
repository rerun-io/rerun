from __future__ import annotations

from ..._baseclasses import DescribedComponentBatch


class VisibleTimeRangeExt:
    """Extension for [VisibleTimeRange][rerun.blueprint.components.VisibleTimeRange]."""

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        """
        Convert into a single element list of component batches as-if it was contained in `rerun.blueprint.archetypes.VisibleTimeRanges`.

        This way this component can be used directly for setting a single time range in overrides.
        """
        from ...blueprint.archetypes import VisibleTimeRanges

        return VisibleTimeRanges([self]).as_component_batches()
