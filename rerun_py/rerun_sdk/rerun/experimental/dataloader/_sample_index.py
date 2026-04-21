"""Pre-computed sample space for deterministic multi-worker sampling."""

from __future__ import annotations

import warnings
from dataclasses import dataclass
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from collections.abc import Iterable

    from ._config import Column, DataSource


def _ns_to_datetime64(ns: int) -> np.datetime64:
    """Convert nanoseconds since epoch to a `datetime64[ns]` scalar."""
    return np.datetime64(ns, "ns")


@dataclass(frozen=True)
class FixedRateSampling:
    """
    Sample timestamp timelines at a fixed nominal rate.

    Indices are drawn on an algebraic grid
    `seg.index_start + k * ns_per_sample`. The server's
    `fill_latest_at` absorbs any drift from real-row positions.
    """

    rate_hz: float


@dataclass(frozen=True)
class SegmentMetadata:
    """Per-segment metadata for sampling."""

    segment_id: str
    index_start: int
    index_end: int
    num_samples: int


class SampleIndex:
    """
    Pre-computed description of the complete sample space.

    Maps every segment's positional indices to concrete index values,
    accounting for the timeline strategy (integer or fixed-rate grid).
    Small enough to hold in memory for any realistic dataset.

    Parameters
    ----------
    segments
        Per-segment metadata (window-adjusted index range + sample count).
    ns_per_sample
        For [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling]: nanoseconds between grid points.
        `None` for integer indices.
    is_timestamp
        True when the index is a timestamp timeline. Controls output
        dtype of `indices_in_range`.

    """

    def __init__(
        self,
        segments: list[SegmentMetadata],
        *,
        ns_per_sample: int | None = None,
        is_timestamp: bool = False,
    ) -> None:
        self._segments = segments
        self._ns_per_sample = ns_per_sample
        self._is_timestamp = is_timestamp

    @property
    def segments(self) -> list[SegmentMetadata]:
        """Per-segment metadata list."""
        return self._segments

    @property
    def is_timestamp(self) -> bool:
        """Whether the index is a timestamp timeline."""
        return self._is_timestamp

    @property
    def ns_per_sample(self) -> int | None:
        """Nanoseconds between grid points for fixed-rate sampling, or ``None``."""
        return self._ns_per_sample

    @property
    def total_samples(self) -> int:
        """Total number of samples across all segments."""
        return sum(s.num_samples for s in self._segments)

    def resolve_local_index(self, seg: SegmentMetadata, pos: int) -> int | np.datetime64:
        """
        Convert a positional index within `seg` to a concrete index value.

        `pos` is in `[0, seg.num_samples)`. Returns `datetime64[ns]`
        for timestamp timelines, a plain `int` for integer indices.
        """
        if self._ns_per_sample is not None:
            ns = seg.index_start + int(pos) * self._ns_per_sample
            return _ns_to_datetime64(ns)
        return int(seg.index_start) + int(pos)

    def indices_in_range(self, seg: SegmentMetadata, lo: int, hi: int) -> Iterable[int]:  # noqa: ARG002
        """
        Enumerate valid index values in `[lo, hi]` for `seg`.

        Returned values are plain `int` (ns-since-epoch for timestamp
        indices). The caller casts the aggregated set to the right
        `numpy` dtype.
        """
        if hi < lo:
            return ()
        if self._ns_per_sample is not None:
            step = self._ns_per_sample
            n = (hi - lo) // step
            return (hi - j * step for j in range(n + 1))
        return range(lo, hi + 1)

    @staticmethod
    def build(
        source: DataSource,
        index: str,
        columns: dict[str, Column],
        *,
        timeline_sampling: FixedRateSampling | None = None,
    ) -> SampleIndex:
        """
        Build a [`SampleIndex`][rerun.experimental.dataloader.SampleIndex] from lightweight metadata queries.

        Parameters
        ----------
        source
            Data source to build from.
        index
            Name of the index timeline column.
        columns
            Column definitions for window-trim calculation.
        timeline_sampling
            Required for timestamp indices; ignored for integer indices.
            Pass [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling] for a regular grid.

        """
        return _build(source, index, columns, timeline_sampling=timeline_sampling)


def _ns_per_sample(rate_hz: float) -> int:
    if rate_hz <= 0:
        raise ValueError(f"FixedRateSampling.rate_hz must be > 0, got {rate_hz}")

    return round(1e9 / rate_hz)


@dataclass(frozen=True)
class _RangesCtx:
    """Parameters shared across the per-segment build loop."""

    columns: dict[str, Column]
    ranges_table: pa.Table
    start_col: str
    end_col: str


def _find_range_columns(ranges_table: pa.Table, index: str) -> tuple[str, str]:
    """
    Find the start/end column names for *index* in a ranges table.

    Looks for columns containing `index` and one of
    `start`/`min` (low end) or `end`/`max` (high end). Raises
    if either side is missing or ambiguous.
    """
    candidates = [n for n in ranges_table.column_names if n != "rerun_segment_id" and index in n]

    def pick(keywords: tuple[str, ...], side: str) -> str:
        matches = [n for n in candidates if any(k in n.lower() for k in keywords)]
        if not matches:
            raise ValueError(
                f"Could not find {side} range column for index {index!r} in columns: {ranges_table.column_names}"
            )
        if len(matches) > 1:
            raise ValueError(f"Ambiguous {side} range column for index {index!r}: {matches}")
        return matches[0]  # type: ignore[no-any-return]

    return pick(("start", "min"), "start"), pick(("end", "max"), "end")


def _window_trims_ns(columns: dict[str, Column]) -> tuple[int, int]:
    """(trim_start, trim_end) from column window offsets (native units)."""
    trim_start = 0
    trim_end = 0
    for col in columns.values():
        if col.window is not None:
            trim_start = max(trim_start, -col.window[0])
            trim_end = max(trim_end, col.window[1])
    return trim_start, trim_end


def _build(
    source: DataSource,
    index: str,
    columns: dict[str, Column],
    *,
    timeline_sampling: FixedRateSampling | None,
) -> SampleIndex:
    """Build a SampleIndex from a DataSource."""
    dataset = source.dataset

    all_segment_ids = dataset.segment_ids()
    if source.segments is not None:
        seg_set = set(source.segments)
        all_segment_ids = [s for s in all_segment_ids if s in seg_set]

    if not all_segment_ids:
        return SampleIndex([])

    view = dataset.filter_segments(all_segment_ids)
    ranges_table = view.get_index_ranges().to_arrow_table()

    start_col, end_col = _find_range_columns(ranges_table, index)
    ctx = _RangesCtx(
        columns=columns,
        ranges_table=ranges_table,
        start_col=start_col,
        end_col=end_col,
    )

    start_type = ranges_table.schema.field(start_col).type
    is_timestamp = pa.types.is_timestamp(start_type)

    if is_timestamp:
        if timeline_sampling is None:
            raise TypeError(
                f"Index {index!r} is a timestamp timeline; you must pass "
                "timeline_sampling=FixedRateSampling(rate_hz=…) so the "
                "dataloader knows how to draw sample indices."
            )
        return _build_fixed_rate(ctx, _ns_per_sample(timeline_sampling.rate_hz))

    if timeline_sampling is not None:
        warnings.warn(
            f"timeline_sampling={timeline_sampling!r} ignored: index {index!r} is not a timestamp timeline",
            stacklevel=3,
        )
    return _build_integer(ctx)


def _build_integer(ctx: _RangesCtx) -> SampleIndex:
    """Build SampleIndex for integer-indexed data."""
    min_window_start = 0
    max_window_end = 0
    for col in ctx.columns.values():
        if col.window is not None:
            min_window_start = min(min_window_start, col.window[0])
            max_window_end = max(max_window_end, col.window[1])

    seg_col = ctx.ranges_table.column("rerun_segment_id").to_pylist()
    min_vals = ctx.ranges_table.column(ctx.start_col).to_pylist()
    max_vals = ctx.ranges_table.column(ctx.end_col).to_pylist()

    segments: list[SegmentMetadata] = []
    for seg_id, seg_min, seg_max in zip(seg_col, min_vals, max_vals, strict=False):
        if seg_min is None or seg_max is None:
            continue
        effective_min = int(seg_min) - min_window_start
        effective_max = int(seg_max) - max_window_end
        if effective_min > effective_max:
            continue

        num_samples = effective_max - effective_min + 1
        segments.append(
            SegmentMetadata(
                segment_id=seg_id,
                index_start=effective_min,
                index_end=effective_max,
                num_samples=num_samples,
            )
        )

    return SampleIndex(segments, ns_per_sample=None, is_timestamp=False)


def _build_fixed_rate(ctx: _RangesCtx, ns_per_sample: int) -> SampleIndex:
    """
    Build SampleIndex for a timestamp timeline sampled at a fixed rate.

    With a user-provided rate we compute `num_samples` and draw
    sample timestamps algebraically on a grid -- no server query for
    distinct timestamps. Any drift between grid and real timestamps
    is absorbed by `fill_latest_at` on the server.
    """
    seg_col = ctx.ranges_table.column("rerun_segment_id").to_pylist()
    min_vals = ctx.ranges_table.column(ctx.start_col).to_numpy()
    max_vals = ctx.ranges_table.column(ctx.end_col).to_numpy()

    trim_start_ns, trim_end_ns = _window_trims_ns(ctx.columns)

    segments: list[SegmentMetadata] = []
    for seg_id, seg_min, seg_max in zip(seg_col, min_vals, max_vals, strict=False):
        if seg_min is None:
            continue
        lo = int(seg_min) + trim_start_ns
        hi = int(seg_max) - trim_end_ns
        if lo > hi:
            continue
        n = (hi - lo) // ns_per_sample + 1
        if n <= 0:
            continue
        segments.append(
            SegmentMetadata(
                segment_id=seg_id,
                index_start=lo,
                index_end=lo + (n - 1) * ns_per_sample,
                num_samples=int(n),
            )
        )

    return SampleIndex(segments, ns_per_sample=ns_per_sample, is_timestamp=True)
