from __future__ import annotations

import pyarrow as pa


def union_discriminant_type(data_type: pa.DenseUnionType, discriminant: str) -> pa.DataType:
    """Return the data type of the given discriminant."""
    return next(f.type for f in list(data_type) if f.name == discriminant)


def build_dense_union(data_type: pa.DenseUnionType, discriminant: str, child: pa.Array) -> pa.Array:
    """
    Build a dense UnionArray given the `data_type`, a discriminant, and the child value array for a single child.

    If the discriminant string doesn't match any possible value, a `ValueError` is raised.
    """

    idx = [f.name for f in list(data_type)].index(discriminant)
    type_ids = pa.array([idx] * len(child), type=pa.int8())
    value_offsets = pa.array(range(len(child)), type=pa.int32())

    children = [pa.nulls(0, type=f.type) for f in list(data_type)]
    try:
        children[idx] = child.cast(data_type[idx].type, safe=False)
    except pa.ArrowInvalid:
        # Since we're having issues with nullability in union types,
        # the cast sometimes fails but can be skipped.
        children[idx] = child

    return pa.Array.from_buffers(
        type=data_type,
        length=len(child),
        buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
        children=children,
    )
