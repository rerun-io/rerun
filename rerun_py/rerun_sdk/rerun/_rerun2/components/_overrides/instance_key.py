from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .. import InstanceKeyArrayLike


def instancekey_native_to_pa_array(data: InstanceKeyArrayLike, data_type: pa.DataType) -> pa.Array:
    array = np.asarray(data, dtype=np.uint64).flatten()
    return pa.array(array, type=data_type)
