from __future__ import annotations

from typing import Final


class TextLogLevelExt:
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
