from __future__ import annotations

import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "DisconnectedSpaceArray",
    "DisconnectedSpaceType",
]


class DisconnectedSpaceArray(pa.ExtensionArray):  # type: ignore[misc]
    @classmethod
    def single(cls) -> DisconnectedSpaceArray:
        """Build a `DisconnectedSpaceArray` with a single element."""

        storage = pa.array([False], type=DisconnectedSpaceType.storage_type)

        # TODO(clement) enable extension type wrapper
        # return cast(DisconnectedSpaceArray, pa.ExtensionArray.from_storage(DisconnectedSpaceType(), storage))
        return storage  # type: ignore[no-any-return]


DisconnectedSpaceType = ComponentTypeFactory(
    "DisconnectedSpaceType", DisconnectedSpaceArray, REGISTERED_COMPONENT_NAMES["rerun.disconnected_space"]
)

pa.register_extension_type(DisconnectedSpaceType())
