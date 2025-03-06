from __future__ import annotations

from typing import Any, overload


class TimeIntExt:
    """Extension for [TimeInt][rerun.datatypes.TimeInt]."""

    @overload
    def __init__(self: Any, *, seq: int) -> None: ...

    @overload
    def __init__(self: Any, *, seconds: float) -> None: ...

    @overload
    def __init__(self: Any, *, nanos: int) -> None: ...

    def __init__(self: Any, *, seq: int | None = None, seconds: float | None = None, nanos: int | None = None) -> None:
        """
        Create a new instance of the TimeInt datatype.

        Exactly one of `seq`, `seconds`, or `nanos` must be provided.

        Parameters
        ----------
        seq:
            Time as a sequence number.

        seconds:
            Time in seconds.

            Interpreted either as a duration or time since unix epoch (depending on timeline type).

        nanos:
            Time in nanoseconds.

            Interpreted either as a duration or time since unix epoch (depending on timeline type).

        """

        if sum(x is not None for x in (seq, seconds, nanos)) != 1:
            raise ValueError("Exactly one of 'seq', 'seconds', or 'nanos' must be provided.")

        if seq is not None:
            self.__attrs_init__(value=seq)
        elif seconds is not None:
            self.__attrs_init__(value=int(seconds * 1e9))
        elif nanos is not None:
            self.__attrs_init__(value=int(nanos))
