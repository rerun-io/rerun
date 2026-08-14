"""The decoder base classes and the batch/request types the pipeline hands them."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from collections.abc import Sequence

    import torch

    from rerun.experimental._selector import Selector

    from .._sample_index import IndexValue


@dataclass(frozen=True, slots=True)
class DecodeRequest:
    """
    One sample's decode request within a field's batch.

    The pipeline has already resolved the index-value window this sample needs
    into batch rows, so a decoder slices `rows` rather than searching for them.
    """

    segment_id: str
    """Segment this sample comes from. Index values are only comparable within one segment."""

    index_value: IndexValue
    """The typed target index value of this sample."""

    rows: range
    """
    The batch rows holding this sample's decode window.

    The sample itself sits at the window's end; any earlier rows are context
    (an explicit `Field.window`, or the prior GOP for compressed video).
    """

    starts_at_keyframe: bool
    """
    Whether the window's first row is known to be a keyframe a decoder may start from.

    True when the field is keyframe-anchored and the pipeline found the prior
    keyframe; false for a plain window start, which carries no such guarantee.
    """


@dataclass(frozen=True, kw_only=True, slots=True, eq=False)
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
    [`DecodeRequest.rows`][rerun.experimental.dataloader.DecodeRequest].
    """

    column: pa.Array
    select: Selector | None = None

    def raw(self, request: DecodeRequest) -> pa.Array:
        """The request's rows as a zero-copy slice, with `select` applied."""
        sliced = self.column.slice(request.rows.start, len(request.rows))
        if self.select is not None:
            out = self.select.execute(sliced)
            if out is None:
                return pa.array([], type=sliced.type)
            return out
        return sliced


class ColumnDecoder(ABC):
    """
    Base class for column decoders.

    Subclasses convert raw Arrow data into tensors. The pipeline calls
    [`decode`][rerun.experimental.dataloader.ColumnDecoder.decode]
    once per field with every requested sample of a fetch block, so decoders can
    amortize work across samples (one vectorized gather for numeric data, one
    codec pass per GOP for video). Decoders that only care about one sample at a
    time can simply loop over the requests and decode each
    [`FieldBatch.raw`][rerun.experimental.dataloader.FieldBatch.raw] window.

    Context-aware decoders (compressed video) should also override
    [`context_range`][rerun.experimental.dataloader.ColumnDecoder.context_range] so the prefetcher fetches surrounding data.
    """

    @abstractmethod
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        """
        Decode all `requests` against `batch`, returning one entry per request.

        `requests` arrive in row order: grouped by segment, and ascending by
        index value within a segment. Implementations may process them in any
        internal order (e.g. grouped by GOP), but the result must align 1:1 with
        the input: `result[i]` is `requests[i]`'s tensor, or `None` to signal
        data missing for that sample.
        """
        ...

    def context_range(
        self,
        index_value: IndexValue,
    ) -> tuple[IndexValue, IndexValue] | None:
        """
        Extra index-value range needed to decode *index_value*.

        Returns `(start, end)` inclusive, or `None` when only the
        exact index value is required (the default).
        """
        del index_value
        return None

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
        most recent value snapped from the real rows. Compressed video keeps it
        `True` too (consecutive duplicates from a dense grid are dropped at
        decode time), but a decoder reading frame-indexed data where the grid
        lands 1:1 on real samples can override to `False` for exact, fill-free
        packet reads. The read is partitioned by this flag so it stays a global
        query argument per group rather than a per-column one.
        """
        return True

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"
