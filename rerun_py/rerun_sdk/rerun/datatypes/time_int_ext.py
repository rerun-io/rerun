from __future__ import annotations

from typing import Any


class TimeIntExt:
    """Extension for [TimeInt][rerun.datatypes.TimeInt]."""

    def __init__(self: Any, *, seq: int | None = None, seconds: float | None = None, nanos: int | None = None) -> None:
        """
        Create a new instance of the TimeInt datatype.

        Parameters
        ----------
        seq:
            Time as a sequence number. Mutually exclusive with seconds and nanos.
        seconds:
            Time in seconds. Mutually exclusive with seq and nanos.
        nanos:
            Time in nanoseconds. Mutually exclusive with seq and seconds.

        """

        if seq is not None:
            if seconds is not None or nanos is not None:
                raise ValueError("Only one of seq, seconds, or nanos can be provided.")
            self.__attrs_init__(value=seq)
        elif seconds is not None:
            if nanos is not None:
                raise ValueError("Only one of seq, seconds, or nanos can be provided.")
            self.__attrs_init__(value=int(seconds * 1e9))
        elif nanos is not None:
            self.__attrs_init__(value=int(nanos))
        else:
            raise ValueError("One of seq, seconds, or nanos must be provided.")
