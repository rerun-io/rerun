"""The components package defines Python wrapper types for common registered Rerun components."""
from __future__ import annotations

from typing import Any, Final, Type, cast

import pyarrow as pa

from rerun import bindings

all = [
    "annotation",
    "arrow",
    "box",
    "color",
    "draw_order",
    "experimental",
    "label",
    "pinhole",
    "point",
    "quaternion",
    "radius",
    "rect2d",
    "scalar_plot_props",
    "scalar",
    "tensor",
    "text_entry",
    "vec",
]

# Component names that are recognized by Rerun.
REGISTERED_COMPONENT_NAMES: Final[dict[str, pa.field]] = bindings.get_registered_component_names()


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


def union_discriminant_type(data_type: pa.DenseUnionType, discriminant: str) -> pa.DataType:
    """Return the data type of the given discriminant."""
    return next(f.type for f in list(data_type) if f.name == discriminant)


def build_dense_union(data_type: pa.DenseUnionType, discriminant: str, child: pa.Array) -> pa.UnionArray:
    """
    Build a dense UnionArray given the `data_type`, a discriminant, and the child value array.

    If the discriminant string doesn't match any possible value, a `ValueError` is raised.

    WARNING: Because of #705, each new union component needs to be handled in `array_to_rust` on the native side.
    """
    try:
        idx = [f.name for f in list(data_type)].index(discriminant)
        type_ids = pa.array([idx] * len(child), type=pa.int8())
        value_offsets = pa.array(range(len(child)), type=pa.int32())

        children = [pa.nulls(0, type=f.type) for f in list(data_type)]
        try:
            children[idx] = child.cast(data_type[idx].type, safe=False)
        except pa.ArrowInvalid:
            # Since we're having issues with nullability in union types (see below),
            # the cast sometimes fails but can be skipped.
            children[idx] = child

        return pa.Array.from_buffers(
            type=data_type,
            length=len(child),
            buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
            children=children,
        )
        # Cast doesn't work for non-flat unions it seems - we're getting issues about the nullability of union variants.
        # It's pointless anyways since on the native side we have to cast the field types
        # See https://github.com/rerun-io/rerun/issues/795
        # .cast(data_type)

    except ValueError as e:
        raise ValueError(e.args)
