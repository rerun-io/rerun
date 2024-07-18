from __future__ import annotations

from typing import Any


class Resolution2DExt:
    """Extension for [Resolution2D][rerun.components.Resolution2D]."""

    def __init__(
        self: Any,
        *,
        width: int,
        height: int,
    ):
        self.__attrs_init__([width, height])
