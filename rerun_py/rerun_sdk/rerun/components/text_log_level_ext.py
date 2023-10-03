from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from . import TextLogLevel


class TextLogLevelExt:
    """Extension for [TextLogLevel][rerun.components.TextLogLevel]."""

    CRITICAL: TextLogLevel = None  # type: ignore[assignment]
    """ Designates catastrophic failures. """

    ERROR: TextLogLevel = None  # type: ignore[assignment]
    """ Designates very serious errors. """

    WARN: TextLogLevel = None  # type: ignore[assignment]
    """ Designates hazardous situations. """

    INFO: TextLogLevel = None  # type: ignore[assignment]
    """ Designates useful information. """

    DEBUG: TextLogLevel = None  # type: ignore[assignment]
    """ Designates lower priority information. """

    TRACE: TextLogLevel = None  # type: ignore[assignment]
    """ Designates very low priority, often extremely verbose, information. """

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.CRITICAL = cls("CRITICAL")
        cls.ERROR = cls("ERROR")
        cls.WARN = cls("WARN")
        cls.INFO = cls("INFO")
        cls.DEBUG = cls("DEBUG")
        cls.TRACE = cls("TRACE")
