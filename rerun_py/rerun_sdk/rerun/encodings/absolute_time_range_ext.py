from __future__ import annotations

from typing import Any

from .. import encodings


def converter(x: encodings.TimeIntLike) -> encodings.TimeInt:
    if isinstance(x, encodings.TimeInt):
        return x
    else:
        return encodings.TimeInt(seq=x)


class AbsoluteTimeRangeExt:
    """Extension for [AbsoluteTimeRange][rerun.encodings.AbsoluteTimeRange]."""

    def __init__(self: Any, min: encodings.TimeIntLike, max: encodings.TimeIntLike) -> None:
        """
        Create a new instance of the AbsoluteTimeRange datatype.

        Parameters
        ----------
        min:
            Beginning of the time range.

        max:
            End of the time range.

        """

        self.__attrs_init__(
            min=converter(min),
            max=converter(max),
        )
