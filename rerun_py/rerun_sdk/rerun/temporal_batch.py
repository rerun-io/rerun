from __future__ import annotations

from typing import Iterable, Protocol, TypeVar

import pyarrow as pa
import rerun_bindings as bindings

from ._baseclasses import Archetype, ComponentBatchLike
from ._log import IndicatorComponentBatch
from .error_utils import catch_and_log_exceptions
from .recording_stream import RecordingStream


class TimeBatchLike(Protocol):
    """Describes interface for objects that can be converted to batch of rerun timepoints."""

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        ...

    def as_arrow_array(self) -> pa.Array:
        """Returns the name of the component."""
        ...


class TimeSequenceBatch(TimeBatchLike):
    """
    A batch of timepoints that are represented as an integer sequence.

    Equivalent to `rr.set_time_sequence`.
    """

    def __init__(self, timeline: str, times: Iterable[int]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.int64())


class TimeSecondsBatch(TimeBatchLike):
    """
    A batch of timepoints that are represented as an floating point seconds.

    Equivalent to `rr.set_time_seconds`.
    """

    def __init__(self, timeline: str, times: Iterable[float]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array([int(t * 1e9) for t in self.times], type=pa.timestamp("ns"))


class TimeNanosBatch(TimeBatchLike):
    """
    A batch of timepoints that are represented as an integer nanoseconds.

    Equivalent to `rr.set_time_nanos`.
    """

    def __init__(self, timeline: str, times: Iterable[int]):
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.timestamp("ns"))


TArchetype = TypeVar("TArchetype", bound=Archetype)


@catch_and_log_exceptions()
def log_temporal_batch(
    entity_path: str,
    times: Iterable[TimeBatchLike],
    components: Iterable[ComponentBatchLike],
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    r"""
    Directly log a temporal batch of data to Rerun.

    Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
    in a columnar form. Each `TimeBatchLike` and `ComponentBatchLike` object represents a column
    of data that will be sent to Rerun. The lengths of all of these columns must match, and all
    data that shares the same index across the different columns will act as a single logical row,
    equivalent to a single call to `rr.log()`.

    Note that this API ignores any stateful time set on the log stream via the `rerun.set_time_*` APIs.

    When using a regular `ComponentBatch` input, the batch data will map to single-valued component
    instance at each timepoint.

    For example, scalars would be logged as:
    ```py
    times = np.arange(0, 64)
    scalars = np.sin(times / 10.0)

    rr.log_temporal_batch(
        "scalars",
        times=[rr.TimeSequenceBatch("step", times)],
        components=[rr.components.ScalarBatch(scalars)],
    )
    ```
    In the viewer this will show up as 64 individual scalar values, one for each timepoint.

    However, it is still possible to log temporal batches of batch data. To do this the source data first must
    be created as a single contiguous batch, and can then be partitioned using the `.partition()` helper on the
    `ComponentBatch` objects.

    For example, to log 5 batches of 20 point clouds, first create a batch of 100 (20 * 5) point clouds and then
    partition it into 5 batches of 20 point clouds:
    ```py
    times = np.arange(0, 5)
    positions = rng.uniform(-5, 5, size=[100, 3])

    rr.log_temporal_batch(
        "points",
        times=[rr.TimeSequenceBatch("step", times)],
        components=[
            rr.Points3D.indicator(),
            rr.components.Position3DBatch(positions).partition([20, 20, 20, 20, 20]),
        ],
    )
    ```
    In the viewer this will show up as 5 individual point clouds, one for each timepoint.

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.

        See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    times:
        The timepoints of this batch of data. Each `TimeBatchLike` object represents a single column
        of timestamps. Generally you should use one of the provided classes: [`TimeSequenceBatch`][],
        [`TimeSecondsBatch`][], or [`TimeNanosBatch`][].
    components:
        The batches of components to log. Each `ComponentBatchLike` object represents a single column of data.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        if None, use the global default from `rerun.strict_mode()`

    """
    expected_length = None

    timelines_args = {}
    for t in times:
        timeline_name = t.timeline_name()
        temporal_batch = t.as_arrow_array()
        if expected_length is None:
            expected_length = len(temporal_batch)
        elif len(temporal_batch) != expected_length:
            raise ValueError(
                f"All times and components in a batch must have the same length. Expected length: {expected_length} but got: {len(temporal_batch)} for timeline: {timeline_name}"
            )

        timelines_args[timeline_name] = temporal_batch

    indicators = []

    components_args = {}
    for c in components:
        if isinstance(c, IndicatorComponentBatch):
            indicators.append(c)
            continue
        component_name = c.component_name()
        temporal_batch = c.as_arrow_array()  # type: ignore[union-attr]
        if expected_length is None:
            expected_length = len(temporal_batch)
        elif len(temporal_batch) != expected_length:
            raise ValueError(
                f"All times and components in a batch must have the same length. Expected length: {expected_length} but got: {len(temporal_batch)} for component: {component_name}"
            )

        components_args[component_name] = temporal_batch

    for i in indicators:
        if expected_length is None:
            expected_length = 1

        components_args[i.component_name()] = pa.nulls(expected_length, type=pa.null())

    bindings.log_arrow_chunk(
        entity_path,
        timelines={t.timeline_name(): t.as_arrow_array() for t in times},
        components=components_args,
        recording=recording,
    )
