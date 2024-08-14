from __future__ import annotations

from typing import Iterable, Protocol, TypeVar, Union

import pyarrow as pa
import rerun_bindings as bindings

from ._baseclasses import Archetype, ComponentBatchMixin, ComponentColumn
from ._log import IndicatorComponentBatch
from .error_utils import catch_and_log_exceptions
from .recording_stream import RecordingStream


class TimeColumnLike(Protocol):
    """Describes interface for objects that can be converted to a column of rerun time values."""

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        ...

    def as_arrow_array(self) -> pa.Array:
        """Returns the name of the component."""
        ...


class TimeSequenceColumn(TimeColumnLike):
    """
    A column of time values that are represented as an integer sequence.

    Columnar equivalent to [`rerun.set_time_sequence`][rerun.set_time_sequence].
    """

    def __init__(self, timeline: str, times: Iterable[int]):
        """
        Create a column of integer sequence time values.

        Parameters
        ----------
        timeline:
            The name of the timeline.
        times:
            An iterable of integer time values.

        """
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.int64())


class TimeSecondsColumn(TimeColumnLike):
    """
    A column of time values that are represented as floating point seconds.

    Columnar equivalent to [`rerun.set_time_seconds`][rerun.set_time_seconds].
    """

    def __init__(self, timeline: str, times: Iterable[float]):
        """
        Create a column of floating point seconds time values.

        Parameters
        ----------
        timeline:
            The name of the timeline.
        times:
            An iterable of floating point second time values.

        """
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array([int(t * 1e9) for t in self.times], type=pa.timestamp("ns"))


class TimeNanosColumn(TimeColumnLike):
    """
    A column of time values that are represented as integer nanoseconds.

    Columnar equivalent to [`rerun.set_time_nanos`][rerun.set_time_nanos].
    """

    def __init__(self, timeline: str, times: Iterable[int]):
        """
        Create a column of integer nanoseconds time values.

        Parameters
        ----------
        timeline:
            The name of the timeline.
        times:
            An iterable of integer nanosecond time values.

        """
        self.timeline = timeline
        self.times = times

    def timeline_name(self) -> str:
        """Returns the name of the timeline."""
        return self.timeline

    def as_arrow_array(self) -> pa.Array:
        return pa.array(self.times, type=pa.timestamp("ns"))


TArchetype = TypeVar("TArchetype", bound=Archetype)


@catch_and_log_exceptions()
def send_columns(
    entity_path: str,
    times: Iterable[TimeColumnLike],
    components: Iterable[Union[ComponentBatchMixin, ComponentColumn]],
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    r"""
    Send columnar data to Rerun.

    Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
    in a columnar form. Each `TimeColumnLike` and `ComponentColumnLike` object represents a column
    of data that will be sent to Rerun. The lengths of all of these columns must match, and all
    data that shares the same index across the different columns will act as a single logical row,
    equivalent to a single call to `rr.log()`.

    Note that this API ignores any stateful time set on the log stream via the `rerun.set_time_*` APIs.
    Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.

    When using a regular `ComponentBatch` input, the batch data will map to single-valued component
    instances at each timepoint.

    For example, scalars would be logged as:
    ```py
    times = np.arange(0, 64)
    scalars = np.sin(times / 10.0)

    rr.send_columns(
        "scalars",
        times=[rr.TimeSequenceColumn("step", times)],
        components=[rr.components.ScalarBatch(scalars)],
    )
    ```
    In the viewer this will show up as 64 individual scalar values, one for each timepoint.

    However, it is still possible to send temporal batches of batch data. To do this the source data first must
    be created as a single contiguous batch, and can then be partitioned using the `.partition()` helper on the
    `ComponentBatch` objects.

    For example, to log 5 batches of 20 point clouds, first create a batch of 100 (20 * 5) point clouds and then
    partition it into 5 batches of 20 point clouds:
    ```py
    times = np.arange(0, 5)
    positions = rng.uniform(-5, 5, size=[100, 3])

    rr.send_columns(
        "points",
        times=[rr.TimeSequenceColumn("step", times)],
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
        The time values of this batch of data. Each `TimeColumnLike` object represents a single column
        of timestamps. Generally, you should use one of the provided classes: [`TimeSequenceColumn`][rerun.TimeSequenceColumn],
        [`TimeSecondsColumn`][rerun.TimeSecondsColumn], or [`TimeNanosColumn`][rerun.TimeNanosColumn].
    components:
        The columns of components to log. Each object represents a single column of data.

        If a batch of components is passed, it will be partitioned with one element per timepoint.
        In order to send multiple components per time value, explicitly create a [`ComponentColumn`][rerun.ComponentColumn]
        either by constructing it directly, or by calling the `.partition()` method on a `ComponentBatch` type.
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
        time_column = t.as_arrow_array()
        if expected_length is None:
            expected_length = len(time_column)
        elif len(time_column) != expected_length:
            raise ValueError(
                f"All times and components in a batch must have the same length. Expected length: {expected_length} but got: {len(time_column)} for timeline: {timeline_name}"
            )

        timelines_args[timeline_name] = time_column

    indicators = []

    components_args = {}
    for c in components:
        if isinstance(c, IndicatorComponentBatch):
            indicators.append(c)
            continue
        component_name = c.component_name()

        if isinstance(c, ComponentColumn):
            component_column = c
        elif isinstance(c, ComponentBatchMixin):
            component_column = c.partition([1] * len(c))  # type: ignore[arg-type]
        else:
            raise TypeError(
                f"Expected either a type that implements the `ComponentMixin` or a `ComponentColumn`, got: {type(c)}"
            )
        arrow_list_array = component_column.as_arrow_array()

        if expected_length is None:
            expected_length = len(arrow_list_array)
        elif len(arrow_list_array) != expected_length:
            raise ValueError(
                f"All times and components in a batch must have the same length. Expected length: {expected_length} but got: {len(arrow_list_array)} for component: {component_name}"
            )

        components_args[component_name] = arrow_list_array

    for i in indicators:
        if expected_length is None:
            expected_length = 1

        components_args[i.component_name()] = pa.nulls(expected_length, type=pa.null())

    bindings.send_arrow_chunk(
        entity_path,
        timelines={t.timeline_name(): t.as_arrow_array() for t in times},
        components=components_args,
        recording=recording,
    )
