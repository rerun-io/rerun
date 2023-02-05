from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.color_conversion import u8_array_to_rgba
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "ColorRGBAArray",
    "ColorRGBAType",
]


class ColorRGBAArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.uint8]) -> ColorRGBAArray:
        """Build a `ColorRGBAArray` from an numpy array."""
        storage = pa.array([u8_array_to_rgba(c) for c in array], type=ColorRGBAType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(ColorRGBAArray, pa.ExtensionArray.from_storage(ColorRGBAType(), storage))
        return storage  # type: ignore[no-any-return]


ColorRGBAType = ComponentTypeFactory("ColorRGBAType", ColorRGBAArray, REGISTERED_COMPONENT_NAMES["rerun.colorrgba"])

pa.register_extension_type(ColorRGBAType())
