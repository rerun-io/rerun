from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "InstanceArray",
    "InstanceType",
]

MAX_U64 = 2**64 - 1


class InstanceArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.uint64]) -> InstanceArray:
        """Build a `InstanceArray` from an numpy array."""
        storage = pa.array(array, type=InstanceType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(InstanceArray, pa.ExtensionArray.from_storage(InstanceType(), storage))
        return storage  # type: ignore[no-any-return]

    def splat() -> InstanceArray:  # type: ignore[misc]
        storage = pa.array([MAX_U64], type=InstanceType.storage_type)
        return storage  # type: ignore[no-any-return]


InstanceType = ComponentTypeFactory("InstanceType", InstanceArray, REGISTERED_COMPONENT_NAMES["rerun.instance_key"])

pa.register_extension_type(InstanceType())
