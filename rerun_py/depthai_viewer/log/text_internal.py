import logging
from dataclasses import dataclass
from typing import Any, Dict, Final, Optional

# Fully qualified to avoid circular import
from depthai_viewer import bindings
from depthai_viewer.components.color import ColorRGBAArray
from depthai_viewer.components.instance import InstanceArray
from depthai_viewer.components.text_entry import TextEntryArray
from depthai_viewer.log import Color, _normalize_colors

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
    level: Optional[str] = LogLevel.INFO,
    color: Optional[Color] = None,
    timeless: bool = False,
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

    """

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if text:
        instanced["rerun.text_entry"] = TextEntryArray.from_bodies_and_levels([(text, level)])
    else:
        logging.warning(f"Null  text entry in log_text_entry('{entity_path}') will be dropped.")

    if color:
        colors = _normalize_colors([color])
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)
