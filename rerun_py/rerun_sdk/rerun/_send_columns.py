from __future__ import annotations

from typing import Iterable, Protocol, TypeVar

import pyarrow as pa
import rerun_bindings as bindings

from ._baseclasses import Archetype, ComponentColumn, ComponentDescriptor
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
    indexes: Iterable[TimeColumnLike],
    columns: Iterable[ComponentColumn],
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    r"""
    Send columnar data to Rerun.

    Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
    in a columnar form. Each `TimeColumnLike` and `ComponentColumn` object represents a column
    of data that will be sent to Rerun. The lengths of all these columns must match, and all
    data that shares the same index across the different columns will act as a single logical row,
    equivalent to a single call to `rr.log()`.

    Note that this API ignores any stateful time set on the log stream via the `rerun.set_time_*` APIs.
    Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.

        See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    indexes:
        The time values of this batch of data. Each `TimeColumnLike` object represents a single column
        of timestamps. Generally, you should use one of the provided classes: [`TimeSequenceColumn`][rerun.TimeSequenceColumn],
        [`TimeSecondsColumn`][rerun.TimeSecondsColumn], or [`TimeNanosColumn`][rerun.TimeNanosColumn].
    columns:
        The columns of components to log. Each object represents a single column of data.

        In order to send multiple components per time value, explicitly create a [`ComponentColumn`][rerun.ComponentColumn]
        either by constructing it directly, or by calling the `.columns()` method on an `Archetype` type.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        If None, use the global default from `rerun.strict_mode()`

    """
    expected_length = None

    timelines_args = {}
    for t in indexes:
        timeline_name = t.timeline_name()
        time_column = t.as_arrow_array()
        if expected_length is None:
            expected_length = len(time_column)
        elif len(time_column) != expected_length:
            raise ValueError(
                f"All times and components in a column must have the same length. Expected length: {expected_length} but got: {len(time_column)} for timeline: {timeline_name}"
            )

        timelines_args[timeline_name] = time_column

    columns_args: dict[ComponentDescriptor, pa.Array] = {}
    for component_column in columns:
        component_descr = component_column.component_descriptor()
        arrow_list_array = component_column.as_arrow_array()
        if expected_length is None:
            expected_length = len(arrow_list_array)
        elif len(arrow_list_array) != expected_length:
            raise ValueError(
                f"All times and components in a column must have the same length. Expected length: {expected_length} but got: {len(arrow_list_array)} for component: {component_descr}"
            )

        columns_args[component_descr] = arrow_list_array

    bindings.send_arrow_chunk(
        entity_path,
        timelines={t.timeline_name(): t.as_arrow_array() for t in indexes},
        components=columns_args,
        recording=recording.to_native() if recording is not None else None,
    )
