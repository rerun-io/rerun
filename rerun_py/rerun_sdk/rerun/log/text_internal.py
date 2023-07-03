from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Final

from rerun import bindings
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.text_entry import TextEntryArray
from rerun.log import Color, _normalize_colors
from rerun.recording_stream import RecordingStream

# Fully qualified to avoid circular import

__all__ = [
    "LogLevel",
    "log_text_entry_internal",
]


@dataclass
class LogLevel:
    """
    Represents the standard log levels.

    This is a collection of constants rather than an enum because we do support
    arbitrary strings as level (e.g. for user-defined levels).
    """

    CRITICAL: Final = "CRITICAL"
    """ Designates catastrophic failures. """

    ERROR: Final = "ERROR"
    """ Designates very serious errors. """

    WARN: Final = "WARN"
    """ Designates hazardous situations. """

    INFO: Final = "INFO"
    """ Designates useful information. """

    DEBUG: Final = "DEBUG"
    """ Designates lower priority information. """

    TRACE: Final = "TRACE"
    """ Designates very low priority, often extremely verbose, information. """


def log_text_entry_internal(
    entity_path: str,
    text: str,
    *,
    level: str | None = LogLevel.INFO,
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
        The level of the text entry (default: `LogLevel.INFO`). Note this can technically
        be an arbitrary string, but it's recommended to use one of the constants
        from [LogLevel][rerun.log.text.LogLevel]
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

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if text:
        instanced["rerun.text_entry"] = TextEntryArray.from_bodies_and_levels([(text, level)])
    else:
        logging.warning(f"Null  text entry in log_text_entry('{entity_path}') will be dropped.")

    if color is not None:
        colors = _normalize_colors(color)
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)
