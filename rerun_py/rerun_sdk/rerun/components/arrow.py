from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory
from rerun.components.vec import Vec3DArray

__all__ = [
    "Arrow3DArray",
    "Arrow3DType",
]


class Arrow3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(origins: npt.NDArray[np.float32], vectors: npt.NDArray[np.float32]) -> Arrow3DArray:
        """Build a `Arrow3DArray` from an Nx3 numpy array."""
        assert origins.shape[1] == 3
        assert origins.shape == vectors.shape

        origins = Vec3DArray.from_numpy(origins)
        vectors = Vec3DArray.from_numpy(vectors)

        storage = pa.StructArray.from_arrays(
            arrays=[origins, vectors],
            fields=list(Arrow3DType.storage_type),
        )
        # TODO(john) enable extension type wrapper
        # return cast(Arrow3DArray, pa.ExtensionArray.from_storage(Arrow3DType(), storage))
        return storage  # type: ignore[no-any-return]


Arrow3DType = ComponentTypeFactory("Arrow3DType", Arrow3DArray, REGISTERED_COMPONENT_NAMES["rerun.arrow3d"])

pa.register_extension_type(Arrow3DType())
