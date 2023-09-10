from __future__ import annotations

from .. import TextLogLevelType

setattr(TextLogLevelType, "CRITICAL", "CRITICAL")
TextLogLevelType.CRITICAL.__doc__ = """ Designates catastrophic failures. """

setattr(TextLogLevelType, "ERROR", "ERROR")
TextLogLevelType.ERROR.__doc__ = """ Designates very serious errors. """

setattr(TextLogLevelType, "WARN", "WARN")
TextLogLevelType.WARN.__doc__ = """ Designates hazardous situations. """

setattr(TextLogLevelType, "INFO", "INFO")
TextLogLevelType.INFO.__doc__ = """ Designates useful information. """

setattr(TextLogLevelType, "DEBUG", "DEBUG")
TextLogLevelType.DEBUG.__doc__ = """ Designates lower priority information. """

setattr(TextLogLevelType, "TRACE", "TRACE")
TextLogLevelType.TRACE.__doc__ = """ Designates very low priority, often extremely verbose, information. """
