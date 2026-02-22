from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np

import rerun_bindings

from .._validators import flat_np_float32_array_from_array_like

if TYPE_CHECKING:
    import pyarrow as pa

    from . import Vec3DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class Vec3DExt:
    """Extension for [Vec3D][rerun.datatypes.Vec3D]."""

    @staticmethod
    def native_to_pa_array_override(data: Vec3DArrayLike, data_type: pa.DataType) -> pa.Array:  # noqa: ARG004
        points = flat_np_float32_array_from_array_like(data, 3)
        points = np.ascontiguousarray(points)
        return rerun_bindings.build_fixed_size_list_array(points, 3)
