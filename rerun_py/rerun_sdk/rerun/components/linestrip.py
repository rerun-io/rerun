from __future__ import annotations

import itertools
from typing import Iterable

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "LineStrip2DArray",
    "LineStrip2DType",
    "LineStrip3DArray",
    "LineStrip3DType",
]


class LineStrip2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy_arrays(array: Iterable[npt.NDArray[np.float32]]) -> LineStrip2DArray:
        """Build a `LineStrip2DArray` from an array of [Nx2 numpy array]."""
        for line in array:
            assert line.shape[1] == 2

        offsets = itertools.chain([0], itertools.accumulate(len(line) for line in array))
        values = np.concatenate(array)  # type: ignore[call-overload]
        fixed = pa.FixedSizeListArray.from_arrays(values.flatten(), type=LineStrip2DType.storage_type.value_type)
        storage = pa.ListArray.from_arrays(offsets, fixed, type=LineStrip2DType.storage_type)

        # TODO(john) enable extension type wrapper
        # return cast(LineStrip2DArray, pa.ExtensionArray.from_storage(LineStrip2DType(), storage))
        return storage  # type: ignore[no-any-return]


LineStrip2DType = ComponentTypeFactory(
    "LineStrip2DType", LineStrip2DArray, REGISTERED_COMPONENT_NAMES["rerun.linestrip2d"]
)

pa.register_extension_type(LineStrip2DType())


class LineStrip3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy_arrays(array: Iterable[npt.NDArray[np.float32]]) -> LineStrip3DArray:
        """Build a `LineStrip3DArray` from an array of [Nx3 numpy array]."""
        for line in array:
            assert line.shape[1] == 3

        offsets = itertools.chain([0], itertools.accumulate(len(line) for line in array))
        values = np.concatenate(array)  # type: ignore[call-overload]
        fixed = pa.FixedSizeListArray.from_arrays(values.flatten(), type=LineStrip3DType.storage_type.value_type)
        storage = pa.ListArray.from_arrays(offsets, fixed, type=LineStrip3DType.storage_type)

        # TODO(john) enable extension type wrapper
        # return cast(LineStrip3DArray, pa.ExtensionArray.from_storage(LineStrip3DType(), storage))
        return storage  # type: ignore[no-any-return]


LineStrip3DType = ComponentTypeFactory(
    "LineStrip3DType", LineStrip3DArray, REGISTERED_COMPONENT_NAMES["rerun.linestrip3d"]
)

pa.register_extension_type(LineStrip3DType())
