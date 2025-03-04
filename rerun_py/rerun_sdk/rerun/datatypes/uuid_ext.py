from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .._converters import to_np_uint8
from .._validators import flat_np_array_from_array_like

if TYPE_CHECKING:
    from . import UuidArrayLike

NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class UuidExt:
    """Extension for [Uuid][rerun.datatypes.Uuid]."""

    @staticmethod
    def native_to_pa_array_override(data: UuidArrayLike, data_type: pa.DataType) -> pa.Array:
        uuids = to_np_uint8(data)  # type: ignore[arg-type]    # Any array like works and Uuid has an __array__ method.
        uuids = flat_np_array_from_array_like(uuids, 16)
        return pa.FixedSizeListArray.from_arrays(uuids, type=data_type)
