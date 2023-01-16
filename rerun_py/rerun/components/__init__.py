"""The components package defines Python wrapper types for common registered Rerun components."""

from __future__ import annotations

from typing import Any, Final, Type, cast

import pyarrow as pa

from rerun import bindings

all = [
    "annotation",
    "box",
    "color",
    "label",
    "point",
    "quaternion",
    "radius",
    "rect2d",
    "scalar",
    "scalar_plot_props",
    "text_entry",
    "vec",
]

REGISTERED_FIELDS: Final[dict[str, pa.field]] = bindings.get_registered_fields()


def ComponentTypeFactory(name: str, array_cls: type[pa.ExtensionArray], field: pa.Field) -> type[pa.ExtensionType]:
    """Build a component type wrapper."""

    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, self.storage_type, field.name)

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        return b""

    @classmethod  # type: ignore[misc]
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        """Return an instance of this subclass given the serialized metadata."""
        return cast(Type[pa.ExtensionType], cls())

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return array_cls

    component_type = type(
        name,
        (pa.ExtensionType,),
        {
            "storage_type": field.type,
            "__init__": __init__,
            "__arrow_ext_serialize__": __arrow_ext_serialize__,
            "__arrow_ext_deserialize__": __arrow_ext_deserialize__,
            "__arrow_ext_class__": __arrow_ext_class__,
        },
    )

    return cast(Type[pa.ExtensionType], component_type)
