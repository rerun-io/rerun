from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import TYPE_CHECKING, overload

import numpy as np
import rerun_bindings as bindings
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

if TYPE_CHECKING:
    from rerun.recording_stream import RecordingStream

# --- Time ---


# These overloads ensure that mypy can catch errors that would otherwise not be caught until runtime.
@overload
def set_index(timeline: str, *, recording: RecordingStream | None = None, sequence: int) -> None: ...


@overload
def set_index(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    timedelta: int | float | timedelta | np.timedelta64,
) -> None: ...


@overload
def set_index(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    datetime: int | float | datetime | np.datetime64,
) -> None: ...


def set_index(
    timeline: str,
    *,
    recording: RecordingStream | None = None,
    sequence: int | None = None,
    timedelta: int | float | timedelta | np.timedelta64 | None = None,
    datetime: int | float | datetime | np.datetime64 | None = None,
) -> None:
    """
    Set the current time of a timeline for this thread.

    Used for all subsequent logging on the same thread, until the next call to
    [`rerun.set_index`][], [`rerun.reset_time`][] or [`rerun.disable_timeline`][].

    For example: `set_index("frame_nr", sequence=frame_nr)`.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    You are expected to set exactly ONE of the arguments `sequence`, `timedelta`, or `datetime`.
    You may NOT change the type of a timeline, so if you use `timedelta` for a specific timeline,
    you must only use `timedelta` for that timeline going forward.

    The columnar equivalent to this function is [`rerun.IndexColumn`][].

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
    timedelta:
        Used for relative times, like `time_since_start`.
        Must either be in seconds, a [`datetime.timedelta`][], or [`numpy.timedelta64`][].
        For nanosecond precision, use `numpy.timedelta64(nanoseconds, 'ns')`.
    datetime:
        Used for absolute time indices, like `capture_time`.
        Must either be in seconds since Unix epoch, a [`datetime.datetime`][], or [`numpy.datetime64`][].
        For nanosecond precision, use `numpy.datetime64(nanoseconds, 'ns')`.

    """
    if sum(x is not None for x in (sequence, timedelta, datetime)) != 1:
        raise ValueError(
            "set_index: Exactly one of `sequence`, `timedelta`, and `datetime` must be set (timeline='{timeline}')",
        )

    if sequence is not None:
        bindings.set_time_sequence(
            timeline,
            sequence,
            recording=recording.to_native() if recording is not None else None,
        )
    elif timedelta is not None:
        nanos = to_nanos(timedelta)
        # TODO(#8635): call a function that is specific to time-deltas
        bindings.set_time_nanos(
            timeline,
            nanos,
            recording=recording.to_native() if recording is not None else None,
        )
    elif datetime is not None:
        nanos = to_nanos_since_epoch(datetime)
        # TODO(#8635): call a function that is specific to absolute times
        bindings.set_time_nanos(
            timeline,
            nanos,
            recording=recording.to_native() if recording is not None else None,
        )


def to_nanos(timedelta_obj: int | float | timedelta | np.timedelta64) -> int:
    if isinstance(timedelta_obj, (int, np.integer)):
        return 1_000_000_000 * int(timedelta_obj)  # Interpret as seconds and convert to nanos
    elif isinstance(timedelta_obj, float):
        return round(1e9 * timedelta_obj)  # Interpret as seconds and convert to nanos
    elif isinstance(timedelta_obj, timedelta):
        return round(1e9 * timedelta_obj.total_seconds())
    elif isinstance(timedelta_obj, np.timedelta64):
        return timedelta_obj.astype("timedelta64[ns]").astype("int64")  # type: ignore[no-any-return]
    else:
        raise TypeError(
            f"set_index: timedelta must be an int, float, timedelta, or numpy.timedelta64 object, got {type(timedelta_obj)}",
        )


def to_nanos_since_epoch(date_time: int | float | datetime | np.datetime64) -> int:
    if isinstance(date_time, (int, np.integer)):
        return 1_000_000_000 * int(date_time)  # Interpret as seconds and convert to nanos
    elif isinstance(date_time, float):
        return round(1e9 * date_time)  # Interpret as seconds and convert to nanos
    elif isinstance(date_time, datetime):
        if date_time.tzinfo is None:
            date_time = date_time.replace(tzinfo=timezone.utc)
        else:
            date_time = date_time.astimezone(timezone.utc)
        epoch = datetime(1970, 1, 1, tzinfo=timezone.utc)
        return round(1e9 * (date_time - epoch).total_seconds())
    elif isinstance(date_time, np.datetime64):
        return date_time.astype("int64")  # type: ignore[no-any-return]
    else:
        raise TypeError(
            f"set_index: datetime must be an int, float, datetime, or numpy.datetime64 object, got {type(date_time)}",
        )


@deprecated(
    """Use `set_index(sequence=…)` instead.
    See: https://www.rerun.io/docs/reference/migration/migration-0-23?speculative-link for more details.""",
)
def set_time_sequence(timeline: str, sequence: int, recording: RecordingStream | None = None) -> None:
    """
    DEPRECATED: Set the current time for this thread as an integer sequence.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For example: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `disable_timeline("frame_nr")`.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    This function marks the timeline as being of a _squential_ type.
    You should not use the temporal functions ([`rerun.set_time_seconds`][], [`rerun.set_time_nanos`][])
    on the same timeline, as that will produce undefined behavior.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    sequence : int
        The current time on the timeline in integer units.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    bindings.set_time_sequence(
        timeline,
        sequence,
        recording=recording.to_native() if recording is not None else None,
    )


@deprecated(
    """Use `set_index(datetime=seconds)` or set_index(timedelta=seconds)` instead.
    See: https://www.rerun.io/docs/reference/migration/migration-0-23?speculative-link for more details.""",
)
def set_time_seconds(timeline: str, seconds: float, recording: RecordingStream | None = None) -> None:
    """
    DEPRECATED: Set the current time for this thread in seconds.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time_seconds`][] or [`rerun.set_time_nanos`][].

    For example: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a timeline again using `disable_timeline("capture_time")`.

    Very large values will automatically be interpreted as seconds since unix epoch (1970-01-01).
    Small values (less than a few years) will be interpreted as relative
    some unknown point in time, and will be shown as e.g. `+3.132s`.

    The bindings has a built-in time which is `log_time`, and is logged as seconds
    since unix epoch.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    This function marks the timeline as being of a _temporal_ type.
    You should not use the sequential function [`rerun.set_time_sequence`][]
    on the same timeline, as that will produce undefined behavior.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    seconds : float
        The current time on the timeline in seconds.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.set_time_seconds(
        timeline,
        seconds,
        recording=recording.to_native() if recording is not None else None,
    )


@deprecated(
    """Use `set_index(datetime=1e-9 * nanos)` or set_index(timedelta=1e-9 * nanos)` instead.
    See: https://www.rerun.io/docs/reference/migration/migration-0-23?speculative-link for more details.""",
)
def set_time_nanos(timeline: str, nanos: int, recording: RecordingStream | None = None) -> None:
    """
    DEPRECATED: Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time_nanos`][] or [`rerun.set_time_seconds`][].

    For example: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a timeline again using `disable_timeline("capture_time")`.

    Very large values will automatically be interpreted as nanoseconds since unix epoch (1970-01-01).
    Small values (less than a few years) will be interpreted as relative
    some unknown point in time, and will be shown as e.g. `+3.132s`.

    The bindings has a built-in time which is `log_time`, and is logged as nanos since
    unix epoch.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    This function marks the timeline as being of a _temporal_ type.
    You should not use the sequential function [`rerun.set_time_sequence`][]
    on the same timeline, as that will produce undefined behavior.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    nanos : int
        The current time on the timeline in nanoseconds.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.set_time_nanos(
        timeline,
        nanos,
        recording=recording.to_native() if recording is not None else None,
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
    until the next call to [`rerun.set_time_nanos`][] or [`rerun.set_time_seconds`][].

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
