from __future__ import annotations

from typing import TYPE_CHECKING, Any, ClassVar

if TYPE_CHECKING:
    from . import TextLogLevel


class TextLogLevelExt:
    """Extension for [TextLogLevel][rerun.components.TextLogLevel]."""

    CRITICAL: ClassVar[TextLogLevel]
    """ Designates catastrophic failures. """

    ERROR: ClassVar[TextLogLevel]
    """ Designates very serious errors. """

    WARN: ClassVar[TextLogLevel]
    """ Designates hazardous situations. """

    INFO: ClassVar[TextLogLevel]
    """ Designates useful information. """

    DEBUG: ClassVar[TextLogLevel]
    """ Designates lower priority information. """

    TRACE: ClassVar[TextLogLevel]
    """ Designates very low priority, often extremely verbose, information. """

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.CRITICAL = cls("CRITICAL")
        cls.ERROR = cls("ERROR")
        cls.WARN = cls("WARN")
        cls.INFO = cls("INFO")
        cls.DEBUG = cls("DEBUG")
        cls.TRACE = cls("TRACE")
