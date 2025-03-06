from __future__ import annotations

from ..._baseclasses import DescribedComponentBatch


class VisibleTimeRangeExt:
    """Extension for [VisibleTimeRange][rerun.blueprint.components.VisibleTimeRange]."""

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        """
        Convert into a single element list of component batches as-if it was contained in `rerun.blueprint.archetypes.VisibleTimeRanges`.

        This way this component can be used directly for setting a single time range in overrides.
        """
        from typing import cast

        from ...blueprint.archetypes import VisibleTimeRanges
        from .. import VisibleTimeRange

        time_range = cast(VisibleTimeRange, self)

        return VisibleTimeRanges([time_range]).as_component_batches()
