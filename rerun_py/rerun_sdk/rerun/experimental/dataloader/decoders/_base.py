"""The decoder base classes and the batch/request types the pipeline hands them."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, Generic, TypeAlias, TypeVar

import numpy as np
import pyarrow as pa
import torch

from .._yuv import Yuv420Frame

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rerun.chunk._selector import Selector

    from .._sample_index import IndexValue

DecodedValue: TypeAlias = torch.Tensor | Yuv420Frame
DecodedResult: TypeAlias = DecodedValue | None
DecodedSample: TypeAlias = dict[str, DecodedResult]

_DecodedT_co = TypeVar("_DecodedT_co", covariant=True)


@dataclass(frozen=True, slots=True)
class DecodeRequest:
    """
    One sample's decode request within a field's batch.

    The pipeline has already resolved the index-value window this sample needs
    into batch rows, so a decoder consumes row indices rather than searching for them.
    """

    sample_position: int
    """Position where this request's result belongs in the decoded fetch block."""

    segment_id: str
    """Segment this sample comes from. Index values are only comparable within one segment."""

    index_value: IndexValue
    """The typed target index value of this sample."""

    decode_row_indices: tuple[int, ...]
    """
    The batch rows holding all data needed to decode this sample.

    For compressed video this includes the intermediate frames needed to decode
    the requested output, even when the field has no explicit window.
    """

    output_row_indices: tuple[int, ...]
    """
    Physical batch rows whose decoded values form this request's output.

    Always non-empty: preparation omits unresolved requests before invoking a decoder.
    """

    starts_at_keyframe: bool
    """
    Whether the window's first row is known to be a keyframe a decoder may start from.

    True when the pipeline found a prior keyframe at the first decode row;
    false when no prior keyframe exists for the requested output.
    """


class FieldBatch:
    """
    One field's rows for a whole fetch block, across every segment it touches.

    Holds the entire fetched window of the field's column, so a decoder can
    process every sample of the block in one vectorized pass instead of one
    call per sample. A shuffled sampler puts almost every sample in a
    different segment, so batching per segment would collapse to a single
    sample per call.

    Rows are ordered by segment, and ascending by index value within a
    segment, so a decoder walking requests in order walks the column forwards.
    Index values are only comparable inside a segment, which is why locating a
    sample's rows is the pipeline's job: see
    [`DecodeRequest`][rerun.experimental.dataloader.DecodeRequest].

    The rows are kept in the chunks they were fetched in. Read them with
    [`rows`][rerun.experimental.dataloader.FieldBatch.rows] or the `take_*`
    methods, which copy at most the rows they return; reading
    [`column`][rerun.experimental.dataloader.FieldBatch.column] copies the whole
    field when it was fetched in several chunks.
    """

    __slots__ = ("_chunk_starts", "_chunks", "_column", "is_windowed", "select")

    def __init__(
        self,
        *,
        column: pa.Array | pa.ChunkedArray,
        select: Selector | None = None,
        is_windowed: bool = False,
    ) -> None:
        """
        Wrap one field's fetched rows.

        Parameters
        ----------
        column:
            A Rerun component column, with one outer Arrow list per timeline row.
        select:
            Selector applied by the `take_*` methods.
        is_windowed:
            Whether decoded outputs keep a leading time axis, including a one-value window.

        """
        if not pa.types.is_list(column.type):
            raise TypeError(
                f"FieldBatch.column must be a Rerun component column with an outer Arrow List type, got {column.type}"
            )
        self._chunks = column if isinstance(column, pa.ChunkedArray) else pa.chunked_array([column])
        self._chunk_starts = np.cumsum([0, *(len(chunk) for chunk in self._chunks.chunks)], dtype=np.int64)
        self._column: pa.Array | None = column if isinstance(column, pa.Array) else None
        self.select = select
        """Selector applied by the `take_*` methods."""
        self.is_windowed = is_windowed
        """Whether decoded outputs keep a leading time axis, including a one-value window."""

    @property
    def column(self) -> pa.Array:
        """
        Every row as one contiguous array.

        When the rows were fetched in several chunks, the first access
        concatenates them: a copy of the whole field that lives as long as the batch.
        """
        if self._column is None:
            chunks = self._chunks
            self._column = chunks.chunk(0) if chunks.num_chunks == 1 else chunks.combine_chunks()
        return self._column

    def rows(self, start: int, stop: int) -> pa.Array:
        """Rows `[start, stop)` as one array: zero-copy within a chunk, a copy of just these rows across chunks."""
        if self._column is not None:
            return self._column.slice(start, stop - start)
        chunk = int(np.searchsorted(self._chunk_starts, start, side="right")) - 1
        if 0 <= chunk < self._chunks.num_chunks and stop <= self._chunk_starts[chunk + 1]:
            return self._chunks.chunk(chunk).slice(start - int(self._chunk_starts[chunk]), stop - start)
        return self._chunks.slice(start, stop - start).combine_chunks()

    def take_decode_rows(self, request: DecodeRequest) -> pa.Array:
        """The request's decode rows, with `select` applied."""
        return self._select(self._take(request.decode_row_indices))

    def take_output_rows(self, request: DecodeRequest) -> pa.Array:
        """The rows requested for output, preserving repeats and applying `select`."""
        out = self._select(self._take(request.output_row_indices))
        if len(out) != len(request.output_row_indices):
            raise ValueError(
                f"Selector returned {len(out)} rows for {len(request.output_row_indices)} requested outputs; "
                "a windowed field requires a selector that preserves row count"
            )
        return out

    def _take(self, row_indices: tuple[int, ...]) -> pa.Array:
        if self._column is not None:
            return self._column.take(pa.array(row_indices, type=pa.int64()))
        # `ChunkedArray.take` concatenates every chunk first, so take from each chunk separately.
        indices = np.asarray(row_indices, dtype=np.int64)
        chunk_ids = np.searchsorted(self._chunk_starts, indices, side="right") - 1
        if indices.size and (chunk_ids.min() < 0 or chunk_ids.max() >= self._chunks.num_chunks):
            raise IndexError(f"Row index out of bounds for a column of {len(self._chunks)} rows")
        # One take per run of consecutive indices into the same chunk, preserving the requested order.
        boundaries = np.flatnonzero(np.diff(chunk_ids)) + 1
        pieces = [
            self._chunks.chunk(int(ids[0])).take(pa.array(run - self._chunk_starts[ids[0]], type=pa.int64()))
            for run, ids in zip(np.split(indices, boundaries), np.split(chunk_ids, boundaries), strict=True)
            if run.size
        ]
        if not pieces:
            return pa.array([], type=self._chunks.type)
        return pieces[0] if len(pieces) == 1 else pa.concat_arrays(pieces)

    def _select(self, sliced: pa.Array) -> pa.Array:
        """Apply this batch's selector to `sliced`."""
        if self.select is not None:
            out = self.select.execute(sliced)
            if out is None:
                return pa.array([], type=sliced.type)
            return out
        return sliced


class ColumnDecoder(ABC, Generic[_DecodedT_co]):
    """
    Base class for column decoders.

    Subclasses convert raw Arrow data into decoded values. The pipeline calls
    [`decode`][rerun.experimental.dataloader.ColumnDecoder.decode]
    once per field with every requested sample of a fetch block, so decoders can
    amortize work across samples (one vectorized gather for numeric data, one
    codec pass per GOP for video). Decoders that only care about one sample at a
    time can simply loop over the requests and decode each
    [`FieldBatch.take_decode_rows`][rerun.experimental.dataloader.FieldBatch.take_decode_rows] window.

    """

    @abstractmethod
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> Sequence[_DecodedT_co | None]:
        """
        Decode all `requests` against `batch`, returning one entry per request.

        `requests` arrive in row order: grouped by segment, and ascending by
        index value within a segment. Implementations may process them in any
        internal order (e.g. grouped by GOP), but the result must align 1:1 with
        the input: `result[i]` is `requests[i]`'s decoded value, or `None` to
        signal data missing for that sample.
        """
        ...

    def prior_keyframe_path(self, field_path: str) -> str | None:
        """
        Sibling column whose non-null rows mark a re-entrant keyframe, or `None`.

        Override on decoders that need the prefetch window anchored at the prior
        keyframe (compressed video). Default returns `None`.
        """
        del field_path
        return None

    @property
    def fill_latest_at(self) -> bool:
        """
        Whether this column's prefetch read latest-at-fills empty grid slots.

        `True` for stateless columns (images, scalars): each grid slot wants the
        most recent value snapped from the real rows. Keyframe-aware fields use
        exact range queries instead, regardless of this value. The read is
        partitioned by this flag so it stays a global query argument per group
        rather than a per-column one.
        """
        return True

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"
