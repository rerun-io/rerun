from __future__ import annotations

import logging

from .components import TextLogLevel


class LoggingHandler(logging.Handler):
    """
    Provides a logging handler that forwards all events to the Rerun SDK.

    [Read more about logging handlers](https://docs.python.org/3/howto/logging.html#handlers).
    """

    LVL2NAME: dict[int, TextLogLevel] = {
        logging.CRITICAL: TextLogLevel.CRITICAL,
        logging.ERROR: TextLogLevel.ERROR,
        logging.WARNING: TextLogLevel.WARN,
        logging.INFO: TextLogLevel.INFO,
        logging.DEBUG: TextLogLevel.DEBUG,
    }

    def __init__(self, path_prefix: str | None = None):
        """
        Initializes the logging handler with an optional path prefix.

        Parameters
        ----------
        path_prefix:
            A common prefix for all logged entity paths.
            Defaults to no prefix.

        """
        logging.Handler.__init__(self)
        self.path_prefix = path_prefix

    def emit(self, record: logging.LogRecord) -> None:
        """Emits a record to the Rerun SDK."""

        from rerun._log import log
        from rerun.archetypes import TextLog

        ent_path = record.module.replace(".", "/")
        if self.path_prefix is not None:
            ent_path = f"{self.path_prefix}/{ent_path}"

        level = self.LVL2NAME.get(record.levelno)
        if level is None:  # user-defined level
            level = TextLogLevel(record.levelname)

        # NOTE: will go to the most appropriate recording!
        return log(ent_path, TextLog(record.getMessage(), level=level))
