from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal, TypeAlias

import numpy as np

from rerun_bindings import format_bytes, format_index_value, format_uint

if TYPE_CHECKING:
    from rerun_bindings import DatasetInfoInternal

TimelineKind: TypeAlias = Literal["sequence", "timestamp", "duration"]
TimelineValue: TypeAlias = np.int64 | np.datetime64 | np.timedelta64


@dataclass(frozen=True)
class LayerInfo:
    """Information about one layer of a dataset."""

    name: str
    num_segments: int
    """Number of distinct segments that have this layer."""

    def __repr__(self) -> str:
        return f"LayerInfo({self._describe()})"

    def _describe(self) -> str:
        return f"{self.name}: in {_count(self.num_segments, 'segment')}"


@dataclass(frozen=True)
class TimelineInfo:
    """Information about one timeline of a dataset, aggregated over its segments."""

    name: str
    kind: TimelineKind
    num_segments: int
    """Number of segments with data on this timeline. May be 0."""

    min: TimelineValue | None
    """Smallest index value across segments, with nanosecond precision for timestamps and durations. `None` when `num_segments == 0`."""

    max: TimelineValue | None
    """Largest index value across segments, with nanosecond precision for timestamps and durations. `None` when `num_segments == 0`."""

    def __repr__(self) -> str:
        return f"TimelineInfo({self._describe()})"

    def _describe(self) -> str:
        if self.min is None or self.max is None:
            return f"{self.name}: {self.kind}, no data"
        bounds = f"{_format_value(self.kind, self.min)} … {_format_value(self.kind, self.max)}"
        return f"{self.name}: {self.kind}, {bounds}, in {_count(self.num_segments, 'segment')}"


@dataclass(frozen=True)
class DatasetInfo:
    """
    Overview of a dataset, as returned by [`DatasetEntry.info`][rerun.catalog.DatasetEntry.info].

    The repr is a compact text overview; the fields are for programmatic use.
    A segment with several layers counts once in `num_segments` and once per layer in `layers`.
    """

    dataset_name: str
    num_segments: int
    size_bytes: int
    layers: frozenset[LayerInfo]
    timelines: frozenset[TimelineInfo]
    num_entities: int
    """Number of entity paths in the dataset schema, excluding properties."""

    num_component_columns: int
    """Number of component columns in the dataset schema, excluding properties."""

    property_columns: frozenset[str]
    """Names of the segment table's property columns."""

    def __repr__(self) -> str:
        header = f"{self.dataset_name}: {_count(self.num_segments, 'segment')}, {format_bytes(self.size_bytes)}"
        entities = _count(self.num_entities, "entity", "entities")
        columns = _count(self.num_component_columns, "component column")
        lines = [
            f"DatasetInfo({header})",
            f"  contents: {entities}, {columns}",
            f"  properties: {', '.join(sorted(self.property_columns)) or '(none)'}",
        ]
        if self.layers:
            lines.append("  layers:")
            lines.extend(f"    {layer._describe()}" for layer in sorted(self.layers, key=lambda layer: layer.name))
        if self.timelines:
            lines.append("  timelines:")
            lines.extend(
                f"    {timeline._describe()}" for timeline in sorted(self.timelines, key=lambda timeline: timeline.name)
            )
        return "\n".join(lines)


def _dataset_info_from_internal(info: DatasetInfoInternal) -> DatasetInfo:
    return DatasetInfo(
        dataset_name=info.dataset_name,
        num_segments=info.num_segments,
        size_bytes=info.size_bytes,
        layers=frozenset(LayerInfo(name=name, num_segments=count) for name, count in info.layers),
        timelines=frozenset(
            TimelineInfo(
                name=name,
                kind=kind,
                num_segments=count,
                min=_to_numpy(kind, min_ns),
                max=_to_numpy(kind, max_ns),
            )
            for name, kind, count, min_ns, max_ns in info.timelines
        ),
        num_entities=info.num_entities,
        num_component_columns=info.num_component_columns,
        property_columns=frozenset(info.property_columns),
    )


def _to_numpy(kind: TimelineKind, value: int | None) -> TimelineValue | None:
    if value is None:
        return None
    if kind == "timestamp":
        return np.datetime64(value, "ns")
    if kind == "duration":
        return np.timedelta64(value, "ns")
    return np.int64(value)


def _count(n: int, singular: str, plural: str | None = None) -> str:
    noun = singular if n == 1 else (plural or f"{singular}s")
    return f"{format_uint(n)} {noun}"


def _format_value(kind: TimelineKind, value: TimelineValue) -> str:
    if isinstance(value, np.datetime64):
        nanos = int(value.astype("datetime64[ns]").astype(np.int64))
    elif isinstance(value, np.timedelta64):
        nanos = int(value.astype("timedelta64[ns]").astype(np.int64))
    else:
        nanos = int(value)
    return format_index_value(kind, nanos)
