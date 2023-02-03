from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "QuaternionArray",
    "QuaternionType",
]


class QuaternionArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float32]) -> QuaternionArray:
        """Build a `QuaternionArray` from an Nx4 numpy array."""
        assert array.shape[1] == 4
        storage = pa.FixedSizeListArray.from_arrays(array.flatten(), type=QuaternionType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(QuaternionArray, pa.ExtensionArray.from_storage(QuaternionType(), storage))
        return storage  # type: ignore[no-any-return]


QuaternionType = ComponentTypeFactory("QuaternionType", QuaternionArray, REGISTERED_COMPONENT_NAMES["rerun.quaternion"])

pa.register_extension_type(QuaternionType())
