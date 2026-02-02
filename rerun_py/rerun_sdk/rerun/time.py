from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import TYPE_CHECKING, overload

import numpy as np
import pyarrow as pa

import rerun_bindings as bindings

if TYPE_CHECKING:
    from rerun.recording_stream import RecordingStream

# --- Time ---


# These overloads ensure that mypy can catch errors that would otherwise not be caught until runtime.
@overload
def set_time(timeline: str, *, recording: RecordingStream | None = None, sequence: int) -> None: ...


@overload
def set_time(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    duration: int | float | timedelta | np.timedelta64,
) -> None: ...


@overload
def set_time(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    timestamp: int | float | datetime | np.datetime64,
) -> None: ...


def set_time(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    sequence: int | None = None,
    duration: int | float | timedelta | np.timedelta64 | None = None,
    timestamp: int | float | datetime | np.datetime64 | pa.TimestampScalar | None = None,
) -> None:
    """
    Set the current time of a timeline for this thread.

    Used for all subsequent logging on the same thread, until the next call to
    [`rerun.set_time`][], [`rerun.reset_time`][] or [`rerun.disable_timeline`][].

    For example: `set_time("frame_nr", sequence=frame_nr)`.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    You are expected to set exactly ONE of the arguments `sequence`, `duration`, or `timestamp`.
    You may NOT change the type of a timeline, so if you use `duration` for a specific timeline,
    you must only use `duration` for that timeline going forward.

    The columnar equivalent to this function is [`rerun.TimeColumn`][].

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording (if there is one).
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    sequence:
        Used for sequential indices, like `frame_nr`.
        Must be an integer.
    duration:
        Used for relative times, like `time_since_start`.
        Must either be in seconds, a [`datetime.timedelta`][], or [`numpy.timedelta64`][].
        For nanosecond precision, use `numpy.timedelta64(nanoseconds, 'ns')`.
    timestamp:
        Used for absolute time indices, like `capture_time`.
        Must either be in seconds since Unix epoch, a [`datetime.datetime`][], or [`numpy.datetime64`][].
        For nanosecond precision, use `numpy.datetime64(nanoseconds, 'ns')`.

    """
    if sum(x is not None for x in (sequence, duration, timestamp)) != 1:
        raise ValueError(
            f"set_time: Exactly one of `sequence`, `duration`, and `timestamp` must be set (timeline='{timeline}')",
        )

    if sequence is not None:
        bindings.set_time_sequence(
            timeline,
            sequence,
            recording=recording.to_native() if recording is not None else None,
        )
    elif duration is not None:
        nanos = to_nanos(duration)
        bindings.set_time_duration_nanos(
            timeline,
            nanos,
            recording=recording.to_native() if recording is not None else None,
        )
    elif timestamp is not None:
        nanos = to_nanos_since_epoch(timestamp)
        bindings.set_time_timestamp_nanos_since_epoch(
            timeline,
            nanos,
            recording=recording.to_native() if recording is not None else None,
        )


def to_nanos(duration: int | np.integer | float | np.float64 | timedelta | np.timedelta64) -> int:
    if isinstance(duration, np.timedelta64):
        return duration.astype("timedelta64[ns]").astype("int64")  # type: ignore[no-any-return]
    elif isinstance(duration, timedelta):
        return round(1e9 * duration.total_seconds())
    elif isinstance(duration, (int, np.integer)):
        return 1_000_000_000 * int(duration)  # Interpret as seconds and convert to nanos
    elif isinstance(
        duration,
        (float, np.floating),
    ):
        return round(1e9 * float(duration))  # Interpret as seconds and convert to nanos
    else:
        raise TypeError(
            f"set_time: duration must be an int, float, timedelta, or numpy.timedelta64 object, got {type(duration)}",
        )


def to_nanos_since_epoch(
    timestamp: int | np.integer | float | np.float64 | datetime | np.datetime64 | pa.TimestampScalar,
) -> int:
    # Only allowing f64 since anything less has way too little precision for measuring time since 1970
    if isinstance(timestamp, (int, np.integer, float, np.float64)):
        if timestamp > 1e11:
            raise ValueError("set_time: Expected seconds since unix epoch, but it looks like this is in milliseconds")
        return int(np.round(1e9 * timestamp))  # Interpret as seconds and convert to nanos
    elif isinstance(timestamp, datetime):
        # Special case pandas to avoid loss of precision
        if hasattr(timestamp, "to_datetime64"):
            datetime64: np.datetime64 = timestamp.to_datetime64()
            return int(datetime64.astype("datetime64[ns]").astype("int64"))

        if timestamp.tzinfo is None:
            timestamp = timestamp.replace(tzinfo=timezone.utc)
        else:
            timestamp = timestamp.astimezone(timezone.utc)
        epoch = datetime(1970, 1, 1, tzinfo=timezone.utc)

        return int(np.round(1e9 * (timestamp - epoch).total_seconds()))
    elif isinstance(timestamp, np.datetime64):
        return int(timestamp.astype("datetime64[ns]").astype("int64"))
    elif isinstance(timestamp, pa.TimestampScalar):
        return int(timestamp.value)
    else:
        raise TypeError(
            f"set_time: timestamp must be an int, float, datetime, numpy.datetime64, or pyarrow.TimestampScalar object, got {type(timestamp)}",
        )


# TODO(emilk): rename to something with the word `index`, and maybe unify with `reset_time`?
def disable_timeline(timeline: str, recording: RecordingStream | None = None) -> None:
    """
    Clear time information for the specified timeline on this thread.

    Parameters
    ----------
    timeline : str
        The name of the timeline to clear the time for.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.disable_timeline(
        timeline,
        recording=recording.to_native() if recording is not None else None,
    )


# TODO(emilk): rename to something with the word `index`, and maybe unify with `disable_timeline`?
def reset_time(recording: RecordingStream | None = None) -> None:
    """
    Clear all timeline information on this thread.

    This is the same as calling `disable_timeline` for all of the active timelines.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time`][].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.reset_time(
        recording=recording.to_native() if recording is not None else None,
    )
