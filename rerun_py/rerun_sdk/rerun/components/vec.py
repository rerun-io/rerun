from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "Vec2DArray",
    "Vec3DArray",
    "Vec2DType",
    "Vec3DType",
]


class Vec2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float32]) -> Vec2DArray:
        """Build a `Vec2DArray` from an Nx2 numpy array."""
        assert len(array) == 0 or array.shape[1] == 2
        storage = pa.FixedSizeListArray.from_arrays(array.flatten(), type=Vec2DType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(Vec2DArray, pa.ExtensionArray.from_storage(Vec2DType(), storage))
        return storage  # type: ignore[no-any-return]


Vec2DType = ComponentTypeFactory("Vec2DType", Vec2DArray, REGISTERED_COMPONENT_NAMES["rerun.vec2d"])

pa.register_extension_type(Vec2DType())


class Vec3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float32]) -> Vec3DArray:
        """Build a `Vec3DArray` from an Nx3 numpy array."""
        assert len(array) == 0 or array.shape[1] == 3
        storage = pa.FixedSizeListArray.from_arrays(array.flatten(), type=Vec3DType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(Vec3DArray, pa.ExtensionArray.from_storage(Vec3DType(), storage))
        return storage  # type: ignore[no-any-return]


Vec3DType = ComponentTypeFactory("Vec3DType", Vec3DArray, REGISTERED_COMPONENT_NAMES["rerun.vec3d"])

pa.register_extension_type(Vec3DType())
