from __future__ import annotations

import numbers
from collections.abc import Sequence, Sized
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import LineStrip2DArrayLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class LineStrip2DExt:
    """Extension for [LineStrip2D][rerun.components.LineStrip2D]."""

    @staticmethod
    def native_to_pa_array_override(data: LineStrip2DArrayLike, data_type: pa.DataType) -> pa.Array:
        _ = data_type  # unused: conversion handled on Rust side
        from ..datatypes import Vec2DBatch
        from . import LineStrip2D

        # pure-numpy fast path
        if isinstance(data, np.ndarray):
            if len(data) == 0:
                return (np.array([], dtype=np.float32), np.array([0], dtype=np.int32), 2)
            elif data.ndim == 2:
                flat = np.ascontiguousarray(np.asarray(data, dtype=np.float32).reshape(-1))
                offsets = np.array([0, len(data)], dtype=np.int32)
                return (flat, offsets, 2)
            else:
                parts = [np.asarray(arr, dtype=np.float32).reshape(-1) for arr in data]
                flat = np.concatenate(parts) if parts else np.array([], dtype=np.float32)
                offsets = np.empty(len(parts) + 1, dtype=np.int32)
                offsets[0] = 0
                for i, p in enumerate(parts):
                    offsets[i + 1] = offsets[i] + len(p) // 2
                return (flat, offsets, 2)

        # pure-object
        elif isinstance(data, LineStrip2D):
            flat = np.ascontiguousarray(np.asarray(data.points, dtype=np.float32).reshape(-1))
            offsets = np.array([0, len(data.points)], dtype=np.int32)
            return (flat, offsets, 2)

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                return (np.array([], dtype=np.float32), np.array([0], dtype=np.int32), 2)

            if isinstance(data[0], Sequence) and len(data[0]) > 0 and isinstance(data[0][0], numbers.Number):
                if len(data[0]) == 2:
                    flat = np.ascontiguousarray(np.asarray(data, dtype=np.float32).reshape(-1))
                    offsets = np.array([0, len(data)], dtype=np.int32)
                    return (flat, offsets, 2)
                else:
                    raise ValueError(
                        "Expected a sequence of sequences of 2D vectors, but the inner sequence length was not equal to 2.",
                    )
            elif isinstance(data[0], np.ndarray) and data[0].shape == (2,):
                flat = np.ascontiguousarray(np.asarray(data, dtype=np.float32).reshape(-1))
                offsets = np.array([0, len(data)], dtype=np.int32)
                return (flat, offsets, 2)
            else:
                parts = []
                for strip in data:
                    if isinstance(strip, LineStrip2D):
                        arr = np.asarray(strip.points, dtype=np.float32).reshape(-1)
                    else:
                        s = np.asarray(strip, dtype=np.float32)
                        if s.ndim != 2 or s.shape[1] != 2:
                            raise ValueError(
                                f"Expected a sequence of 2D vectors, instead got array with shape {s.shape}.",
                            )
                        arr = s.reshape(-1)
                    parts.append(arr)

                flat = np.concatenate(parts) if parts else np.array([], dtype=np.float32)
                offsets = np.empty(len(parts) + 1, dtype=np.int32)
                offsets[0] = 0
                for i, p in enumerate(parts):
                    offsets[i + 1] = offsets[i] + len(p) // 2
                return (flat, offsets, 2)
        else:
            # Fallback: build via Vec2DBatch + pa
            inner = Vec2DBatch(data).as_arrow_array()
            offsets = pa.array([0, len(inner)], type=pa.int32())
            return pa.ListArray.from_arrays(offsets, inner, type=data_type)
