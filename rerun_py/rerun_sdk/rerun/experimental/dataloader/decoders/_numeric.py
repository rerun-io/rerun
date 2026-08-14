"""Decoder for Arrow numeric and list-of-numeric columns."""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import torch

from rerun._tracing import with_tracing

from ._arrow import _flatten_to_numpy_with_offsets, _unwrap_to_numpy
from ._base import ColumnDecoder

if TYPE_CHECKING:
    from collections.abc import Sequence

    from ._base import DecodeRequest, FieldBatch


class NumericDecoder(ColumnDecoder):
    """
    Decode Arrow numeric / list-of-numeric columns to tensors, one vectorized gather per batch.

    Segment-blind: every request already carries its rows, so the gather runs
    across the whole fetch block at once rather than once per sample.
    """

    @with_tracing("NumericDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        flat, offsets = (None, None) if batch.select is not None else _flatten_to_numpy_with_offsets(batch.column)
        if flat is None or offsets is None:
            # Selector set, or a layout the offsets math doesn't cover (null rows).
            return [torch.as_tensor(_unwrap_to_numpy(batch.raw(request))) for request in requests]

        count = len(requests)
        starts = np.fromiter((request.rows.start for request in requests), dtype=np.int64, count=count)
        stops = np.fromiter((request.rows.stop for request in requests), dtype=np.int64, count=count)
        value_starts = offsets[starts]
        lengths = offsets[stops] - value_starts

        if lengths.size and (lengths == lengths[0]).all():
            # Uniform windows: gather every sample at once, hand out row views of the result.
            gather = value_starts[:, None] + np.arange(lengths[0], dtype=np.int64)
            return list(torch.from_numpy(flat[gather]).unbind(0))

        # Ragged windows: copy each slice out so samples own their memory —
        # overlapping windows must not share elements, and a retained sample
        # must not pin the whole fetch buffer. Matches the uniform path's copy.
        flat_tensor = torch.from_numpy(flat)
        return [
            flat_tensor[start : start + length].clone() for start, length in zip(value_starts, lengths, strict=True)
        ]
