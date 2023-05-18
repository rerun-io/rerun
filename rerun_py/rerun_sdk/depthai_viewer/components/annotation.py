from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "ClassIdArray",
    "ClassIdType",
    "KeypointIdArray",
    "KeypointIdType",
]


class ClassIdArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.uint16]) -> ClassIdArray:
        """Build a `ClassIdArray` from an Nx1 numpy array."""
        assert len(array.shape) == 1

        storage = pa.array(array, type=ClassIdType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(ClassIdArray, pa.ExtensionArray.from_storage(ClassIdArray(), storage))
        return storage  # type: ignore[no-any-return]


ClassIdType = ComponentTypeFactory("ClassIdType", ClassIdArray, REGISTERED_COMPONENT_NAMES["rerun.class_id"])


class KeypointIdArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.uint16]) -> KeypointIdArray:
        """Build a `KeypointIdArray` from an Nx1 numpy array."""
        assert len(array.shape) == 1

        storage = pa.array(array, type=KeypointIdType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(KeypointIdArray, pa.ExtensionArray.from_storage(KeypointIdArray(), storage))
        return storage  # type: ignore[no-any-return]


KeypointIdType = ComponentTypeFactory("KeypointIdType", ClassIdArray, REGISTERED_COMPONENT_NAMES["rerun.keypoint_id"])

pa.register_extension_type(ClassIdType())
