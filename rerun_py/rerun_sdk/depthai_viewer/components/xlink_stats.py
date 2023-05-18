from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "XLinkStats"
]

# ---

class XLinkStats(pa.ExtensionArray): # type: ignore[misc]
    def create(
        total_bytes_written: int,
        total_bytes_read: int,
    ) -> "XLinkStats":
        """Build XLinkStats data from total bytes written and read."""
        return pa.StructArray.from_arrays(  # type: ignore[no-any-return]
            fields=XLinkStatsType.storage_type,
            arrays=[[total_bytes_written], [total_bytes_read]],
            mask=pa.array([False, False], type=pa.bool_()),
    )


XLinkStatsType = ComponentTypeFactory("XLinkStatsType", XLinkStats, REGISTERED_COMPONENT_NAMES["rerun.xlink_stats"])

pa.register_extension_type(XLinkStatsType())
