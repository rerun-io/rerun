from __future__ import annotations

from collections.abc import Sequence, Sized
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import BlobArrayLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class BlobExt:
    """Extension for [Blob][rerun.datatypes.Blob]."""

    @staticmethod
    def native_to_pa_array_override(data: BlobArrayLike, data_type: pa.DataType) -> pa.Array:
        _ = data_type  # unused: conversion handled on Rust side
        from ..datatypes import Blob, BlobBatch

        # someone or something is building things manually, let them!
        if isinstance(data, BlobBatch):
            return data.as_arrow_array()

        def _to_uint8(arr: np.ndarray) -> np.ndarray:  # type: ignore[type-arg]
            return np.asarray(arr, dtype=np.uint8).ravel()

        # pure-numpy fast path
        if isinstance(data, np.ndarray):
            if len(data) == 0:
                return (np.array([], dtype=np.uint8), np.array([0], dtype=np.int32), 0)
            elif data.ndim == 1:
                flat = _to_uint8(data)
                offsets = np.array([0, len(flat)], dtype=np.int32)
                return (flat, offsets, 0)
            else:
                parts = [_to_uint8(arr) for arr in data]
                flat = np.concatenate(parts)
                offsets = np.empty(len(parts) + 1, dtype=np.int32)
                offsets[0] = 0
                for i, p in enumerate(parts):
                    offsets[i + 1] = offsets[i] + len(p)
                return (flat, offsets, 0)

        # pure-object
        elif isinstance(data, Blob):
            flat = _to_uint8(np.array(data.data))
            return (flat, np.array([0, len(flat)], dtype=np.int32), 0)

        # pure-bytes
        elif isinstance(data, bytes):
            flat = np.frombuffer(data, dtype=np.uint8)
            return (flat, np.array([0, len(flat)], dtype=np.int32), 0)

        elif hasattr(data, "read"):
            flat = np.frombuffer(data.read(), dtype=np.uint8)
            return (flat, np.array([0, len(flat)], dtype=np.int32), 0)

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                return (np.array([], dtype=np.uint8), np.array([0], dtype=np.int32), 0)
            elif isinstance(data[0], Blob):
                parts = [_to_uint8(np.array(datum.data)) for datum in data]  # type: ignore[union-attr]
            elif isinstance(data[0], bytes):
                parts = [np.frombuffer(datum, dtype=np.uint8) for datum in data]  # type: ignore[arg-type]
            else:
                parts = [_to_uint8(np.array(datum)) for datum in data]

            flat = np.concatenate(parts) if parts else np.array([], dtype=np.uint8)
            offsets = np.empty(len(parts) + 1, dtype=np.int32)
            offsets[0] = 0
            for i, p in enumerate(parts):
                offsets[i + 1] = offsets[i] + len(p)
            return (flat, offsets, 0)

        else:
            flat = _to_uint8(np.array(data.data))
            return (flat, np.array([0, len(flat)], dtype=np.int32), 0)
