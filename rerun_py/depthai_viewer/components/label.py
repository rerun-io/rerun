from __future__ import annotations

from typing import Sequence

import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "LabelArray",
    "LabelType",
]


class LabelArray(pa.ExtensionArray):  # type: ignore[misc]
    def new(labels: Sequence[str]) -> LabelArray:
        """Build a `LabelArray` from a sequence of str."""
        storage = pa.array(labels, type=LabelType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(LabelArray, pa.ExtensionArray.from_storage(LabelType(), storage))
        return storage  # type: ignore[no-any-return]


LabelType = ComponentTypeFactory("LabelType", LabelArray, REGISTERED_COMPONENT_NAMES["rerun.label"])

pa.register_extension_type(LabelType())
