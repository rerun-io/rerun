from __future__ import annotations

from typing import Any

import numpy as np

from ..datatypes import Vec2DArrayLike


class Boxes2DExt:
    def __init__(
        self: Any,
        *,
        sizes: Vec2DArrayLike | None = None,
        mins: Vec2DArrayLike | None = None,
        centers: Vec2DArrayLike | None = None,
        half_sizes: Vec2DArrayLike | None = None,
        **kwargs: Any,
    ) -> None:
        if sizes is not None:
            if half_sizes is not None:
                raise ValueError("Cannot specify both `sizes` and `half_sizes` at the same time.")

            sizes = np.asarray(sizes, dtype=np.float32)
            half_sizes = sizes / 2.0

        if mins is not None:
            if centers is not None:
                raise ValueError("Cannot specify both `mins` and `centers` at the same time.")

            # already converted `sizes` to `half_sizes`
            if half_sizes is None:
                raise ValueError("Cannot specify `mins` without `sizes` or `half_sizes`.")

            mins = np.asarray(mins, dtype=np.float32)
            half_sizes = np.asarray(half_sizes, dtype=np.float32)
            centers = mins + half_sizes

        self.__attrs_init__(half_sizes=half_sizes, centers=centers, **kwargs)
