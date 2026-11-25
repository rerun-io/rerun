"""Pre-computed sample space for deterministic multi-worker sampling."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, TypeAlias

import numpy as np
import pyarrow as pa

from rerun._tracing import tracing_scope, with_tracing

from ._config import FixedRateSampling, TimeAlignedSampling

if TYPE_CHECKING:
    from collections.abc import Iterable

    from rerun.experimental.spans import Curation

    from ._config import DataSource, Field

IndexValue: TypeAlias = int | np.datetime64 | np.timedelta64
"""
A concrete index value on a timeline.

A plain `int` for integer timelines, a `datetime64[ns]` for timestamp
timelines, and a `timedelta64[ns]` for duration timelines.
"""


def _ns_to_datetime64(ns: int) -> np.datetime64:
    """Convert nanoseconds since epoch to a `datetime64[ns]` scalar."""
    return np.datetime64(ns, "ns")


def _ns_to_timedelta64(ns: int) -> np.timedelta64:
    """Convert a nanosecond count to a `timedelta64[ns]` scalar."""
    return np.timedelta64(ns, "ns")


def _ns_to_dtype(ns: int, ns_dtype: str | None) -> IndexValue:
    """Convert a nanosecond count to the index-typed scalar (`int`, `datetime64`, or `timedelta64`)."""
    if ns_dtype == "datetime64[ns]":
        return _ns_to_datetime64(ns)
    if ns_dtype == "timedelta64[ns]":
        return _ns_to_timedelta64(ns)
    return ns


def _index_value_to_int(value: IndexValue) -> int:
    """The raw integer of an index value: nanoseconds for timestamps and durations."""
    if isinstance(value, (np.datetime64, np.timedelta64)):
        return int(value.astype(np.int64))
    return int(value)


@dataclass(frozen=True)
class SegmentMetadata:
    """One contiguous eligible grid range within a segment."""

    segment_id: str
    index_start: int
    index_end: int
    num_samples: int
    index_min: int | None = None
    """Untrimmed recording minimum, or `None` when selected spans do not provide it."""


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
    ns_dtype
        Numpy dtype string used when materializing index values:
        `"datetime64[ns]"` for timestamp timelines, `"timedelta64[ns]"`
        for duration timelines, or `None` for plain integer indices.

    """

    def __init__(
        self,
        segments: list[SegmentMetadata],
        *,
        ns_per_sample: int | None = None,
        ns_dtype: str | None = None,
    ) -> None:
        self._segments = segments
        self._ns_per_sample = ns_per_sample
        self._ns_dtype = ns_dtype

        if sum(s.num_samples for s in segments) > np.iinfo(np.int64).max:
            raise OverflowError("The candidate sample count exceeds int64 capacity")
        seg_sizes = np.array([s.num_samples for s in segments], dtype=np.int64)
        self._cumulative_sizes = np.concatenate([[0], np.cumsum(seg_sizes)])

    @property
    def segments(self) -> list[SegmentMetadata]:
        """Eligible ranges; a segment may have several disjoint ranges."""
        return self._segments

    @property
    def ns_dtype(self) -> str | None:
        """Numpy dtype for materialized index values, or ``None`` for integer indices."""
        return self._ns_dtype

    @property
    def is_timestamp(self) -> bool:
        """Whether the index is a timestamp timeline."""
        return self._ns_dtype == "datetime64[ns]"

    @property
    def is_duration(self) -> bool:
        """Whether the index is a duration timeline."""
        return self._ns_dtype == "timedelta64[ns]"

    @property
    def ns_per_sample(self) -> int | None:
        """Nanoseconds between grid points for fixed-rate sampling, or ``None``."""
        return self._ns_per_sample

    @property
    def total_samples(self) -> int:
        """Total number of samples across all segments."""
        return int(self._cumulative_sizes[-1])

    @property
    def segment_offsets(self) -> np.ndarray:
        """Cumulative offsets: `segments[i]` covers global indices `[segment_offsets[i], segment_offsets[i + 1])`."""
        return self._cumulative_sizes

    def global_to_local(self, idx: int) -> tuple[SegmentMetadata, IndexValue]:
        """
        Map a global index `[0, total_samples)` to `(segment, concrete_idx_value)`.

        The returned index value is a plain `int` for integer timelines,
        a `datetime64[ns]` for timestamp timelines, and a
        `timedelta64[ns]` for duration timelines.
        """
        total = int(self._cumulative_sizes[-1])
        if idx < 0 or idx >= total:
            raise IndexError(f"Index {idx} out of range [0, {total})")
        seg_idx = int(np.searchsorted(self._cumulative_sizes[1:], idx, side="right"))
        pos = idx - int(self._cumulative_sizes[seg_idx])
        seg = self._segments[seg_idx]
        return seg, self.resolve_local_index(seg, pos)

    def resolve_local_index(self, seg: SegmentMetadata, pos: int) -> IndexValue:
        """
        Convert a positional index within `seg` to a concrete index value.

        `pos` is in `[0, seg.num_samples)`. Returns `datetime64[ns]`
        for timestamp timelines, `timedelta64[ns]` for duration
        timelines, and a plain `int` for integer indices.
        """
        if self._ns_per_sample is not None:
            ns = seg.index_start + int(pos) * self._ns_per_sample
            return _ns_to_dtype(ns, self._ns_dtype)
        return int(seg.index_start) + int(pos)

    def indices_in_range(self, lo: int, hi: int) -> Iterable[int]:
        """
        Enumerate valid index values in `[lo, hi]`.

        For fixed-rate timelines the returned values walk down from `hi`
        in `ns_per_sample` steps (so they remain on the grid as long as
        `hi` is). For integer timelines, every value in `[lo, hi]` is
        returned. Values are plain `int` (ns-since-epoch for timestamp
        indices, ns count for duration indices); the caller casts the
        aggregated set to the right `numpy` dtype.
        """
        if hi < lo:
            return ()
        if self._ns_per_sample is not None:
            step = self._ns_per_sample
            n = (hi - lo) // step
            return (hi - j * step for j in range(n + 1))
        return range(lo, hi + 1)

    def offset_index(self, index_value: IndexValue, offset: int | float) -> IndexValue:
        """Add an explicit field-window offset to an index value."""
        if self._ns_dtype is not None:
            base = int(np.asarray(index_value, dtype=self._ns_dtype).astype(np.int64).item())
            value = base + round(float(offset) * 1e9)
            return _ns_to_dtype(value, self._ns_dtype)
        if int(offset) != offset:
            raise ValueError(f"Integer timelines require integral window offsets, got {offset!r}")
        return _index_value_to_int(index_value) + int(offset)

    def output_index_values(self, index_value: IndexValue, field: Field) -> tuple[IndexValue, ...]:
        """Concrete index values requested by a field, preserving its explicit offset order."""
        if field.window is None:
            return (index_value,)
        return tuple(self.offset_index(index_value, offset) for offset in field.window)

    @staticmethod
    @with_tracing("SampleIndex.build")
    def build(
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
    ) -> SampleIndex:
        """
        Build a [`SampleIndex`][rerun.experimental.dataloader.SampleIndex] from lightweight metadata queries.

        Parameters
        ----------
        source
            Data source to build from.
        index
            Name of the index timeline column.
        fields
            Field definitions, used for window-trim calculation.
        timeline_sampling
            [`TimeAlignedSampling`][rerun.experimental.dataloader.TimeAlignedSampling] for aligned integer fields,
            or [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling] for timestamps and durations.
            `None` uses ordinary integer sampling.

        """
        return _build(source, index, fields, timeline_sampling=timeline_sampling)

    @staticmethod
    def from_curation(
        curation: Curation,
        fields: dict[str, Field],
        *,
        timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
    ) -> SampleIndex:
        """
        Derive candidates from selected inclusive intervals without reading training fields.

        Intervals select anchor timestamps; field windows may read outside them.
        Overlapping selections do not duplicate anchors.
        """
        curation = curation.materialize()
        table = curation.to_arrow()
        return _from_bounds(
            segment_ids=table["segment_id"].to_pylist(),
            starts=table["start"].cast(pa.int64()).to_pylist(),
            ends=table["end"].cast(pa.int64()).to_pylist(),
            index_type=curation.index_type,
            fields=fields,
            timeline_sampling=timeline_sampling,
            trim_windows=False,
        )


def _ns_per_sample(rate_hz: float) -> int:
    return round(1e9 / rate_hz)


def _validate_timeline_sampling(
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None, sample_index: SampleIndex
) -> None:
    """Require an explicit sampling mode to match the index timeline and grid."""
    if timeline_sampling is None:
        return
    if isinstance(timeline_sampling, TimeAlignedSampling):
        if sample_index.ns_dtype is not None or sample_index.ns_per_sample is not None:
            raise ValueError("TimeAlignedSampling requires an integer timeline")
    elif isinstance(timeline_sampling, FixedRateSampling):
        if sample_index.ns_dtype is None or sample_index.ns_per_sample is None:
            raise ValueError("FixedRateSampling requires a timestamp or duration timeline")
        if _ns_per_sample(timeline_sampling.rate_hz) != sample_index.ns_per_sample:
            raise ValueError("The sampling rate must match the sample index's grid")
    else:
        raise TypeError("timeline_sampling must be TimeAlignedSampling, FixedRateSampling, or None")


class EmptyTableError(Exception):
    pass


def _find_range_columns(column_names: list[str], index: str) -> tuple[str, str]:
    """
    Find the start/end column names for *index* among range-table columns.

    Looks for columns containing `index` and one of
    `start`/`min` (low end) or `end`/`max` (high end). Raises
    if either side is missing or ambiguous.
    """
    exact = (f"{index}:start", f"{index}:end")
    if all(name in column_names for name in exact):
        return exact
    candidates = [n for n in column_names if n != "rerun_segment_id" and index in n]

    def pick(keywords: tuple[str, ...], side: str) -> str:
        matches = [n for n in candidates if any(k in n.lower() for k in keywords)]
        if not matches:
            raise EmptyTableError(f"Could not find {side} range column for index {index!r} in columns: {column_names}")
        if len(matches) > 1:
            raise ValueError(f"Ambiguous {side} range column for index {index!r}: {matches}")
        return matches[0]  # type: ignore[no-any-return]

    return pick(("start", "min"), "start"), pick(("end", "max"), "end")


def _window_trims_ns(fields: dict[str, Field]) -> tuple[int, int]:
    """
    Largest negative and positive explicit time offsets across all fields.

    Used to shrink the iterable range so windowed lookups stay inside
    each segment. Only called for timestamp or duration timelines, where
    `field.window` is interpreted as seconds and converted to nanoseconds.
    """
    trim_start = 0
    trim_end = 0
    for field in fields.values():
        if field.window is not None:
            trim_start = max(trim_start, -round(min(field.window) * 1e9))
            trim_end = max(trim_end, round(max(field.window) * 1e9))
    return trim_start, trim_end


def _build(
    source: DataSource,
    index: str,
    fields: dict[str, Field],
    *,
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None,
) -> SampleIndex:
    """Build a SampleIndex from recording metadata or resolved curated intervals."""
    if source.curation is not None:
        from rerun.experimental.spans import Curation

        curation, recording_minima = Curation._from_source(source._selection(), index=index, spans=source.curation)
        table = curation.to_arrow()
        return _from_bounds(
            segment_ids=table["segment_id"].to_pylist(),
            starts=table["start"].cast(pa.int64()).to_pylist(),
            ends=table["end"].cast(pa.int64()).to_pylist(),
            index_type=curation.index_type,
            fields=fields,
            timeline_sampling=timeline_sampling,
            recording_minima=recording_minima,
            trim_windows=False,
        )

    if source.segments == []:
        return SampleIndex([])

    ranges_df = source._selection().get_index_ranges()
    try:
        start_col, end_col = _find_range_columns(ranges_df.schema().names, index)
    except EmptyTableError:
        return SampleIndex([])

    ranges_df = ranges_df.select("rerun_segment_id", start_col, end_col).sort("rerun_segment_id")
    with tracing_scope("ranges_df.to_arrow_table"):
        # The lazy server query and deterministic sort run during materialization.
        ranges_table = ranges_df.to_arrow_table()
    segment_ids = ranges_table["rerun_segment_id"].to_pylist()
    starts = ranges_table[start_col].cast(pa.int64()).to_pylist()
    return _from_bounds(
        segment_ids=segment_ids,
        starts=starts,
        ends=ranges_table[end_col].cast(pa.int64()).to_pylist(),
        index_type=ranges_table.schema.field(start_col).type,
        fields=fields,
        timeline_sampling=timeline_sampling,
        recording_minima=dict(zip(segment_ids, starts, strict=True)),
    )


def _from_bounds(
    *,
    segment_ids: list[str],
    starts: list[int | None],
    ends: list[int | None],
    index_type: pa.DataType,
    fields: dict[str, Field],
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None,
    recording_minima: dict[str, int | None] | None = None,
    trim_windows: bool = True,
) -> SampleIndex:
    is_timestamp = pa.types.is_timestamp(index_type)
    is_duration = pa.types.is_duration(index_type)
    if is_timestamp or is_duration:
        if index_type.unit != "ns":
            raise ValueError("Temporal sampling requires nanosecond timeline bounds")
        if isinstance(timeline_sampling, TimeAlignedSampling):
            raise ValueError("TimeAlignedSampling requires an integer timeline")
        if not isinstance(timeline_sampling, FixedRateSampling):
            kind = "timestamp" if is_timestamp else "duration"
            raise TypeError(f"A {kind} timeline requires timeline_sampling=FixedRateSampling(rate_hz=…)")
        step = _ns_per_sample(timeline_sampling.rate_hz)
        ns_dtype = "datetime64[ns]" if is_timestamp else "timedelta64[ns]"
        trim_start, trim_end = _window_trims_ns(fields)
    else:
        if index_type != pa.int64():
            raise ValueError(f"Unsupported timeline type {index_type}")
        if isinstance(timeline_sampling, FixedRateSampling):
            raise ValueError("FixedRateSampling requires a timestamp or duration timeline")
        step = 1
        ns_dtype = None
        trim_start = 0
        trim_end = 0
        for field in fields.values():
            if field.window is not None:
                if any(int(offset) != offset for offset in field.window):
                    raise ValueError(f"Integer timelines require integral window offsets, got {field.window!r}")
                trim_start = max(trim_start, -int(min(field.window)))
                trim_end = max(trim_end, int(max(field.window)))

    eligible: list[tuple[str, int, int]] = []
    for segment_id, start, end in zip(segment_ids, starts, ends, strict=True):
        if start is None or end is None:
            continue
        lo, hi = start, end
        if trim_windows:
            lo += trim_start
            hi -= trim_end
        else:
            lo = -(-lo // step) * step
        hi = lo + ((hi - lo) // step) * step
        if lo <= hi:
            if not trim_windows:
                if lo - trim_start <= -(1 << 63) or hi + trim_end >= 1 << 63:
                    raise ValueError("Curated field windows must fit within the valid 64-bit timeline")
            eligible.append((segment_id, lo, hi))

    merged: list[tuple[str, int, int]] = []
    for segment_id, lo, hi in sorted(eligible):
        if merged and merged[-1][0] == segment_id and lo <= merged[-1][2] + step:
            previous = merged[-1]
            merged[-1] = (segment_id, previous[1], max(previous[2], hi))
        else:
            merged.append((segment_id, lo, hi))
    sample_index = SampleIndex(
        [
            SegmentMetadata(
                segment_id,
                lo,
                hi,
                (hi - lo) // step + 1,
                index_min=recording_minima.get(segment_id) if recording_minima is not None else None,
            )
            for segment_id, lo, hi in merged
        ],
        ns_per_sample=step if ns_dtype is not None else None,
        ns_dtype=ns_dtype,
    )
    _validate_timeline_sampling(timeline_sampling, sample_index)
    return sample_index
