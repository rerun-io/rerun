from __future__ import annotations

from typing import cast
from rerun.color_conversion import u8_array_to_rgba
from rerun.components import ComponentTypeFactory, REGISTERED_FIELDS

import pyarrow as pa


class ColorRGBAArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array) -> ColorRGBAArray:
        """Build a `ColorRGBAArray` from an numpy array."""

        storage = pa.array([u8_array_to_rgba(c) for c in array], type=ColorRGBAType.storage_type)
        return cast(ColorRGBAArray, pa.ExtensionArray.from_storage(ColorRGBAType(), storage))


ColorRGBAType = ComponentTypeFactory("ColorRGBAType", ColorRGBAArray, REGISTERED_FIELDS["colorrgba"])

pa.register_extension_type(ColorRGBAType())
