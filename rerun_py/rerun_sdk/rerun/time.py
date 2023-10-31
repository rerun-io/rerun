from __future__ import annotations

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun.recording_stream import RecordingStream

# --- Time ---


def set_time_sequence(timeline: str, sequence: int, recording: RecordingStream | None = None) -> None:
    """
    Set the current time for this thread as an integer sequence.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For example: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `disable_timeline("frame_nr")`.

    There is no requirement of monotonicity. You can move the time backwards if you like.

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
    recording = RecordingStream.to_native(recording)
    bindings.set_time_sequence(timeline, sequence, recording=recording)


def set_time_seconds(timeline: str, seconds: float, recording: RecordingStream | None = None) -> None:
    """
    Set the current time for this thread in seconds.

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

    recording = RecordingStream.to_native(recording)
    bindings.set_time_seconds(timeline, seconds, recording=recording)


def set_time_nanos(timeline: str, nanos: int, recording: RecordingStream | None = None) -> None:
    """
    Set the current time for this thread.

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

    recording = RecordingStream.to_native(recording)

    bindings.set_time_nanos(timeline, nanos, recording=recording)


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

    recording = RecordingStream.to_native(recording)

    bindings.disable_timeline(timeline, recording=recording)


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

    recording = RecordingStream.to_native(recording)

    bindings.reset_time(recording=recording)
