from __future__ import annotations

from typing import Any

from .. import datatypes


def converter(x: datatypes.TimeIntLike) -> datatypes.TimeInt:
    if isinstance(x, datatypes.TimeInt):
        return x
    else:
        return datatypes.TimeInt(seq=x)


class AbsoluteTimeRangeExt:
    """Extension for [AbsoluteTimeRange][rerun.datatypes.AbsoluteTimeRange]."""

    def __init__(self: Any, min: datatypes.TimeIntLike, max: datatypes.TimeIntLike) -> None:
        """
        Create a new instance of the AbsoluteTimeRange datatype.

        Parameters
        ----------
        min:
            Beginning of the time range.

        max:
            End of the time range.

        """

        self.min = converter(min)
        self.max = converter(max)
