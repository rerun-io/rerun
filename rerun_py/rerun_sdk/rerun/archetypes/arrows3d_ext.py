from __future__ import annotations

from typing import Any

from ..datatypes import Vec3DArrayLike


class Arrows3DExt:
    def __init__(
        self: Any,
        *,
        vectors: Vec3DArrayLike,
        **kwargs: Any,
    ) -> None:
        # Custom constructor to remove positional arguments and force use of keyword arguments
        # while still making vectors required.
        self.__attrs_init__(vectors=vectors, **kwargs)
