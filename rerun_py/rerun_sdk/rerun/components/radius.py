from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "RadiusArray",
    "RadiusType",
]


class RadiusArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float32]) -> RadiusArray:
        """Build a `RadiusArray` from an numpy array."""
        storage = pa.array(array, type=RadiusType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(RadiusArray, pa.ExtensionArray.from_storage(RadiusType(), storage))
        return storage  # type: ignore[no-any-return]


RadiusType = ComponentTypeFactory("RadiusType", RadiusArray, REGISTERED_COMPONENT_NAMES["rerun.radius"])

pa.register_extension_type(RadiusType())
