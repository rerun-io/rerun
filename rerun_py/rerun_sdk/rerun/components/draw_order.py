from __future__ import annotations

import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "DrawOrderArray",
    "DrawOrder",
]


class DrawOrderArray(pa.ExtensionArray):  # type: ignore[misc]
    def splat(draw_order: float) -> DrawOrderArray:  # type: ignore[misc]
        storage = pa.array([draw_order], type=DrawOrder.storage_type)
        return storage  # type: ignore[no-any-return]


DrawOrder = ComponentTypeFactory("DrawOrder", DrawOrderArray, REGISTERED_COMPONENT_NAMES["rerun.draw_order"])

pa.register_extension_type(DrawOrder())
