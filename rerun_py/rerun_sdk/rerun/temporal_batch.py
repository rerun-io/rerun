from __future__ import annotations

from typing import Iterable, Protocol, TypeVar

import pyarrow as pa
import rerun_bindings as bindings

from ._baseclasses import Archetype, ComponentBatchLike
from ._log import IndicatorComponentBatch


class TimeBatchLike(Protocol):
    """Describes interface for objects that can be converted to batch of rerun Components."""

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        ...

    def as_arrow_array(self) -> pa.Array:
        """Returns the name of the component."""
        ...


class TimeSequenceBatch(TimeBatchLike):
    def __init__(self, timeline: str, times: Iterable[int]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.int64())


class TimeSecondsBatch(TimeBatchLike):
    def __init__(self, timeline: str, times: Iterable[float]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array([int(t * 1e9) for t in self.times], type=pa.timestamp("ns"))


class TimeNanosBatch(TimeBatchLike):
    def __init__(self, timeline: str, times: Iterable[int]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.timestamp("ns"))


TArchetype = TypeVar("TArchetype", bound=Archetype)


def log_temporal_batch(
    entity_path: str,
    times: Iterable[TimeBatchLike],
    components: Iterable[ComponentBatchLike | Iterable[ComponentBatchLike]],
) -> None:
    """
    Log a temporal batch of data.

    Parameters
    ----------
    entity_path:
        The entity path to log.
    times:
        The time data to log.
    components:
        The components to log.

    """
    expected_length = None

    timelines_args = {}
    for t in times:
        temporal_batch = t.as_arrow_array()
        if expected_length is None:
            expected_length = len(temporal_batch)
        elif len(temporal_batch) != expected_length:
            raise ValueError("All times and components in a batch must have the same length.")

        timelines_args[t.timeline_name()] = temporal_batch

    indicators = []

    components_args = {}
    for c in components:
        if isinstance(c, IndicatorComponentBatch):
            indicators.append(c)
            continue
        if hasattr(c, "component_name"):
            temporal_batch = c.as_arrow_array()  # type: ignore[union-attr]
            if expected_length is None:
                expected_length = len(temporal_batch)
            elif len(temporal_batch) != expected_length:
                raise ValueError("All times and components in a batch must have the same length.")

            components_args[c.component_name()] = temporal_batch
        else:
            # TODO(jleibs): We can probably code-gen some helpers to make this more efficient.
            # Would be better to extend an existing array than produce a new one.
            component_name = None
            arrays = []
            offsets = []
            accum = 0
            offsets.append(accum)
            for cc in c:
                if component_name is None:
                    component_name = cc.component_name()
                elif component_name != cc.component_name():
                    raise ValueError("All components in a batch must have the same component name.")
                arrays.append(cc.as_arrow_array())
                accum += len(arrays[-1])
                offsets.append(accum)
            if component_name is None:
                raise ValueError("Empty batch of components.")
            if expected_length is None:
                expected_length = len(arrays)
            elif len(arrays) != expected_length:
                raise ValueError("All times and components in a batch must have the same length.")
            temporal_batch = pa.concat_arrays(arrays)

            components_args[component_name] = pa.ListArray.from_arrays(offsets=offsets, values=temporal_batch)

    for i in indicators:
        if expected_length is None:
            expected_length = 1

        components_args[i.component_name()] = pa.nulls(expected_length, type=pa.null())

    bindings.log_arrow_chunk(
        entity_path,
        timelines={t.timeline_name(): t.as_arrow_array() for t in times},
        components=components_args,
    )
