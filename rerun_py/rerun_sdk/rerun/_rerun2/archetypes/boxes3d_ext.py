from __future__ import annotations

from typing import Any

import numpy as np

from ..datatypes import Vec3DArrayLike


class Boxes3DExt:
    def __init__(
        self: Any, *, sizes: Vec3DArrayLike | None = None, mins: Vec3DArrayLike | None = None, **kwargs: Any
    ) -> None:
        if sizes is not None:
            if kwargs.get("half_sizes") is not None:
                raise ValueError("Cannot specify both `sizes` and `half_sizes` at the same time.")

            sizes = np.asarray(sizes, dtype=np.float32)
            kwargs["half_sizes"] = sizes / 2.0

        if mins is not None:
            if kwargs.get("centers") is not None:
                raise ValueError("Cannot specify both `mins` and `centers` at the same time.")

            # already converted `sizes` to `half_sizes`
            if kwargs.get("half_sizes") is None:
                raise ValueError("Cannot specify `mins` without `sizes` or `half_sizes`.")

            mins = np.asarray(mins, dtype=np.float32)
            half_sizes = np.asarray(kwargs["half_sizes"], dtype=np.float32)
            kwargs["centers"] = mins + half_sizes

        self.__attrs_init__(**kwargs)
