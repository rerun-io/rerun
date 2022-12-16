"""The components package defines Python wrapper types for common registered Rerun components."""

from __future__ import annotations

from typing import Any, Final, Type, cast
from rerun import rerun_bindings  # type: ignore[attr-defined]
import pyarrow as pa

all = ["color", "rect2d"]

REGISTERED_FIELDS: Final = rerun_bindings.get_registered_fields()


def ComponentTypeFactory(name: str, array_cls: Type[pa.ExtensionArray], field: pa.Field) -> Type[pa.ExtensionType]:
    """Build a component type wrapper."""

    def __init__(self: Type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, self.storage_type, field.name)

    def __arrow_ext_serialize__(self: Type[pa.ExtensionType]) -> bytes:
        return b""

    @classmethod  # type: ignore[misc]
    def __arrow_ext_deserialize__(
        cls: Type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> Type[pa.ExtensionType]:
        """Return an instance of this subclass given the serialized metadata."""
        return cast(Type[pa.ExtensionType], cls())

    def __arrow_ext_class__(self: Type[pa.ExtensionType]) -> Type[pa.ExtensionArray]:
        return array_cls

    component_type = type(
        name,
        (pa.ExtensionType,),
        dict(
            storage_type=field.type,
            __init__=__init__,
            __arrow_ext_serialize__=__arrow_ext_serialize__,
            __arrow_ext_deserialize__=__arrow_ext_deserialize__,
            __arrow_ext_class__=__arrow_ext_class__,
        ),
    )

    return cast(Type[pa.ExtensionType], component_type)
