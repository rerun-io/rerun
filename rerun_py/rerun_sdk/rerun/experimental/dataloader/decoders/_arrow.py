"""Arrow layout helpers shared by the decoders."""

from __future__ import annotations

from typing import cast

import numpy as np
import pyarrow as pa


def _flatten_to_numpy_with_offsets(arr: pa.Array) -> tuple[np.ndarray, np.ndarray] | tuple[None, None]:
    """
    Flatten `arr` once, with the flat-value offsets of each of its rows.

    Returns `(flat, offsets)` where row `i` occupies `flat[offsets[i]:offsets[i + 1]]`,
    or `(None, None)` when the layout doesn't support the mapping (null rows at any list level),
    in which case callers fall back to per-row decoding.
    """
    # `FieldBatch` guarantees the outer Rerun component-instance list. Start
    # from its explicit row offsets; only nested value layouts vary from here.
    outer = cast("pa.ListArray", arr)
    if outer.null_count != 0:
        return None, None
    outer_offsets = outer.offsets.to_numpy().astype(np.int64)
    offsets = outer_offsets - outer_offsets[0]
    inner = outer.flatten()
    while _is_list_type(inner.type):
        if inner.null_count != 0:
            return None, None
        if pa.types.is_fixed_size_list(inner.type):
            offsets = offsets * inner.type.list_size
        else:
            layer = inner.offsets.to_numpy().astype(np.int64)
            offsets = layer[offsets] - layer[0]
        inner = inner.flatten()
    if inner.null_count != 0:
        return None, None
    # `inner` is the fully flattened values array, laid out exactly as `offsets`
    # indexes it; converting anything else here would desync the two.
    return _unwrap_to_numpy(inner), offsets


def _unwrap_to_numpy(arr: pa.Array) -> np.ndarray:
    """
    Recursively unwrap nested Arrow list types to a numpy array.

    Handles `list<double>`, `list<list<double>>`,
    `fixed_size_list<float>`, and plain numeric arrays.
    """
    if _is_list_type(arr.type):
        # `flatten()` respects the slice's offsets, unlike `.values`.
        inner = arr.flatten()
        if _is_list_type(inner.type):
            return _unwrap_to_numpy(inner)
        arr = inner

    # Torch requires writeable arrays; a zero-copy view into the Arrow buffer is not.
    numpy_array = arr.to_numpy(zero_copy_only=False)
    if not numpy_array.flags.writeable:
        numpy_array = numpy_array.copy()
    return numpy_array  # type: ignore[no-any-return]


def _is_list_type(t: pa.DataType) -> bool:
    return bool(pa.types.is_list(t) or pa.types.is_large_list(t) or pa.types.is_fixed_size_list(t))


def _flatten_blob(arr: pa.Array, row: int) -> np.ndarray:
    """Extract row *row* bytes from a `list<list<uint8>>` or `list<binary | large_binary>` array."""
    outer_offsets = arr.offsets.to_numpy()
    lo, hi = int(outer_offsets[row]), int(outer_offsets[row + 1])
    inner = arr.values.slice(lo, hi - lo)

    if _is_list_type(inner.type):
        # `flatten()` respects the slice's offsets, unlike `.values`.
        return inner.flatten().to_numpy(zero_copy_only=False)  # type: ignore[no-any-return]

    # BinaryArray rows are contiguous in the values buffer; slice via offsets.
    offset_dtype = np.int64 if pa.types.is_large_binary(inner.type) else np.int32
    offsets = np.frombuffer(inner.buffers()[1], dtype=offset_dtype)
    start = int(offsets[inner.offset])
    end = int(offsets[inner.offset + len(inner)])
    return np.frombuffer(inner.buffers()[2], dtype=np.uint8, offset=start, count=end - start)
