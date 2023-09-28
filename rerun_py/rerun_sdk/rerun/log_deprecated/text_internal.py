from __future__ import annotations

from rerun._log import log
from rerun.archetypes import TextLog
from rerun.log_deprecated import Color, _normalize_colors
from rerun.recording_stream import RecordingStream

# Fully qualified to avoid circular import

__all__ = [
    "log_text_entry_internal",
]


def log_text_entry_internal(
    entity_path: str,
    text: str,
    *,
    level: str | None = "INFO",
    color: Color | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Internal API to log a text entry, with optional level.

    This implementation doesn't support extension components, or the exception-capturing decorator
    and is intended to be used from inside the other rerun log functions.

    Parameters
    ----------
    entity_path:
        The object path to log the text entry under.
    text:
        The text to log.
    level:
        The level of the text entry, e.g. "INFO" "ERROR", ….
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    timeless:
        Whether the text entry should be timeless.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    if color is not None:
        color = _normalize_colors(color)

    return log(entity_path, TextLog(text, level=level), timeless=timeless, recording=recording)
