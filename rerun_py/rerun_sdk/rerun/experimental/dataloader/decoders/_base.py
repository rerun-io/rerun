"""The decoder base classes and the batch/request types the pipeline hands them."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, Generic, TypeAlias, TypeVar

import pyarrow as pa
import torch

from .._yuv import Yuv420Frame

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rerun.experimental._selector import Selector

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
    [`DecodeRequest`][rerun.experimental.dataloader.DecodeRequest].
    """

    column: pa.Array
    """A Rerun component column, with one outer Arrow list per timeline row."""

    select: Selector | None = None
    is_windowed: bool = False
    """Whether decoded outputs keep a leading time axis, including a one-value window."""

    def __post_init__(self) -> None:
        if not pa.types.is_list(self.column.type):
            raise TypeError(
                "FieldBatch.column must be a Rerun component column with an outer Arrow List type, "
                f"got {self.column.type}"
            )

    def take_decode_rows(self, request: DecodeRequest) -> pa.Array:
        """The request's decode rows, with `select` applied."""
        selected = self.column.take(pa.array(request.decode_row_indices, type=pa.int64()))
        return self._select(selected)

    def take_output_rows(self, request: DecodeRequest) -> pa.Array:
        """The rows requested for output, preserving repeats and applying `select`."""
        selected = self.column.take(pa.array(request.output_row_indices, type=pa.int64()))
        out = self._select(selected)
        if len(out) != len(request.output_row_indices):
            raise ValueError(
                f"Selector returned {len(out)} rows for {len(request.output_row_indices)} requested outputs; "
                "a windowed field requires a selector that preserves row count"
            )
        return out

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
