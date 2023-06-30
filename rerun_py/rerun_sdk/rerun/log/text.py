from __future__ import annotations

import logging
from typing import Any, Final

import rerun.log.extension_components
from rerun import bindings
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.text_entry import TextEntryArray
from rerun.log import Color, _normalize_colors
from rerun.log.log_decorator import log_decorator
from rerun.log.text_internal import LogLevel
from rerun.recording_stream import RecordingStream

# Fully qualified to avoid circular import

__all__ = [
    "LogLevel",
    "LoggingHandler",
    "log_text_entry",
]


class LoggingHandler(logging.Handler):
    """
    Provides a logging handler that forwards all events to the Rerun SDK.

    Because Rerun's data model doesn't match 1-to-1 with the different concepts from
    python's logging ecosystem, we need a way to map the latter to the former:

    Mapping
    -------
    * Root Entity: Optional root entity to gather all the logs under.

    * Entity path: the name of the logger responsible for the creation of the LogRecord
                   is used as the final entity path, appended after the Root Entity path.

    * Level: the log level is mapped as-is.

    * Body: the body of the text entry corresponds to the formatted output of
            the LogRecord using the standard formatter of the logging package,
            unless it has been overridden by the user.

    [Read more about logging handlers](https://docs.python.org/3/howto/logging.html#handlers)

    """

    LVL2NAME: Final = {
        logging.CRITICAL: LogLevel.CRITICAL,
        logging.ERROR: LogLevel.ERROR,
        logging.WARNING: LogLevel.WARN,
        logging.INFO: LogLevel.INFO,
        logging.DEBUG: LogLevel.DEBUG,
    }

    def __init__(self, root_entity_path: str | None = None):
        logging.Handler.__init__(self)
        self.root_entity_path = root_entity_path

    def emit(self, record: logging.LogRecord) -> None:
        """Emits a record to the Rerun SDK."""
        objpath = record.name.replace(".", "/")
        if self.root_entity_path is not None:
            objpath = f"{self.root_entity_path}/{objpath}"
        level = self.LVL2NAME.get(record.levelno)
        if level is None:  # user-defined level
            level = record.levelname
        # NOTE: will go to the most appropriate recording!
        log_text_entry(objpath, record.getMessage(), level=level)


@log_decorator
def log_text_entry(
    entity_path: str,
    text: str,
    *,
    level: str | None = LogLevel.INFO,
    color: Color | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a text entry, with optional level.

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
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
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

    if ext:
        rerun.log.extension_components._add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)
