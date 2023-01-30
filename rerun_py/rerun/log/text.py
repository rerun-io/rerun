import logging
from dataclasses import dataclass
from typing import Final, Optional, Sequence

from rerun.log import _normalize_colors

from rerun import bindings

__all__ = [
    "LogLevel",
    "LoggingHandler",
    "log_text_entry",
]


@dataclass
class LogLevel:
    """
    Represents the standard log levels.

    This is a collection of constants rather than an enum because we do support
    arbitrary strings as level (e.g. for user-defined levels).
    """

    # """ Designates catastrophic failures. """
    CRITICAL: Final = "CRITICAL"
    # """ Designates very serious errors. """
    ERROR: Final = "ERROR"
    # """ Designates hazardous situations. """
    WARN: Final = "WARN"
    # """ Designates useful information. """
    INFO: Final = "INFO"
    # """ Designates lower priority information. """
    DEBUG: Final = "DEBUG"
    # """ Designates very low priority, often extremely verbose, information. """
    TRACE: Final = "TRACE"


class LoggingHandler(logging.Handler):
    """
    Provides a logging handler that forwards all events to the Rerun SDK.

    Because Rerun's data model doesn't match 1-to-1 with the different concepts from
    python's logging ecosystem, we need a way to map the latter to the former:

    * Root Object: Optional root object to gather all the logs under.

    * Entity path: the name of the logger responsible for the creation of the LogRecord
                   is used as the final entity path, appended after the Root Entity path.

    * Level: the log level is mapped as-is.

    * Body: the body of the text entry corresponds to the formatted output of
            the LogRecord using the standard formatter of the logging package,
            unless it has been overridden by the user.

    Read more about logging handlers at https://docs.python.org/3/howto/logging.html#handlers.
    """

    LVL2NAME: Final = {
        logging.CRITICAL: LogLevel.CRITICAL,
        logging.ERROR: LogLevel.ERROR,
        logging.WARNING: LogLevel.WARN,
        logging.INFO: LogLevel.INFO,
        logging.DEBUG: LogLevel.DEBUG,
    }

    def __init__(self, root_obj_path: Optional[str] = None):
        logging.Handler.__init__(self)
        self.root_obj_path = root_obj_path

    def emit(self, record: logging.LogRecord) -> None:
        """Emits a record to the Rerun SDK."""
        objpath = record.name.replace(".", "/")
        if self.root_obj_path is not None:
            objpath = f"{self.root_obj_path}/{objpath}"
        level = self.LVL2NAME.get(record.levelno)
        if level is None:  # user-defined level
            level = record.levelname
        log_text_entry(objpath, record.getMessage(), level=level)


def log_text_entry(
    obj_path: str,
    text: str,
    level: Optional[str] = LogLevel.INFO,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a text entry, with optional level.

    * If no `level` is given, it will default to `LogLevel.INFO`.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    """
    from rerun.components.color import ColorRGBAArray
    from rerun.components.text_entry import TextEntryArray

    comps = {}
    if text:
        comps["rerun.text_entry"] = TextEntryArray.from_bodies_and_levels([(text, level)])
    else:
        logging.warning(f"Null  text entry in log_text_entry('{obj_path}') will be dropped.")

    if color:
        colors = _normalize_colors([color])
        comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    bindings.log_arrow_msg(obj_path, components=comps, timeless=timeless)
