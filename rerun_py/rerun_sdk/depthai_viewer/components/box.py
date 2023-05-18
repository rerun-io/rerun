from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "Box3DArray",
    "Box3DType",
]


class Box3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float32]) -> Box3DArray:
        """Build a `Box3DArray` from an Nx3 numpy array."""
        assert array.shape[1] == 3
        storage = pa.FixedSizeListArray.from_arrays(array.flatten(), type=Box3DType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(Box3DArray, pa.ExtensionArray.from_storage(Box3DType(), storage))
        return storage  # type: ignore[no-any-return]


Box3DType = ComponentTypeFactory("Box3DType", Box3DArray, REGISTERED_COMPONENT_NAMES["rerun.box3d"])

pa.register_extension_type(Box3DType())
