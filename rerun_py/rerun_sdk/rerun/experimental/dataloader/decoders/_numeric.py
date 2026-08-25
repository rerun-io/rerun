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


class NumericDecoder(ColumnDecoder[torch.Tensor]):
    """
    Decode Arrow numeric / list-of-numeric columns to tensors, one vectorized gather per batch.

    Segment-blind: every request already carries its rows, so the gather runs
    across the whole fetch block at once rather than once per sample.

    Windowed numeric lists require every resolved row to have the same width
    and return `[T, D]` (including `[T, 1]` for scalar components); a window
    with varying widths returns `None`.
    Unwindowed variable-width fields return one tensor per sample and require a padding
    or ragged-data collator for batching.
    """

    @with_tracing("NumericDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        flat, offsets = (None, None) if batch.select is not None else _flatten_to_numpy_with_offsets(batch.column)
        # Selectors and null-containing layouts cannot use flattened row offsets.
        if flat is None or offsets is None:
            return [self._decode_selected(batch, request) for request in requests]

        row_groups = [request.output_row_indices for request in requests]
        row_widths = np.diff(offsets)
        output_widths = np.asarray(
            [row_widths[row] for rows in row_groups for row in rows],
            dtype=np.int64,
        )
        uniform_output_width = output_widths.size > 0 and bool(np.all(output_widths == output_widths[0]))
        if batch.is_windowed and output_widths.size and not uniform_output_width:
            return [self._decode_selected(batch, request) for request in requests]

        # Common fixed-width case, such as joint states.
        if uniform_output_width and output_widths[0] > 0:
            # Uniform rows form one rectangular `[request, output, value]` gather.
            row_indices = np.asarray(row_groups, dtype=np.int64)
            row_width = int(output_widths[0])
            value_indices = offsets[row_indices, np.newaxis] + np.arange(row_width, dtype=np.int64)
            gathered = torch.from_numpy(flat[value_indices])
            if not batch.is_windowed:
                gathered = gathered.reshape(len(requests), -1)
            return list(gathered.unbind(0))

        flat_tensor = torch.from_numpy(flat)

        if not batch.is_windowed:
            return [flat_tensor[offsets[rows[0]] : offsets[rows[0] + 1]].clone() for rows in row_groups]

        if not output_widths.size:
            return []

        row_width = int(output_widths[0])
        value_offsets = np.arange(row_width, dtype=np.int64)
        outputs: list[torch.Tensor | None] = []
        for output_rows in row_groups:
            value_indices = offsets[np.asarray(output_rows), np.newaxis] + value_offsets
            tensor = flat_tensor[value_indices].clone()
            outputs.append(tensor)
        return outputs

    @staticmethod
    def _decode_selected(batch: FieldBatch, request: DecodeRequest) -> torch.Tensor | None:
        output_rows = batch.take_output_rows(request)
        if output_rows.null_count:
            return None
        if not batch.is_windowed:
            return torch.as_tensor(_unwrap_to_numpy(output_rows))
        values = [torch.as_tensor(_unwrap_to_numpy(output_rows.slice(row, 1))) for row in range(len(output_rows))]
        if values and any(value.shape != values[0].shape for value in values[1:]):
            return None
        return torch.stack(values)
