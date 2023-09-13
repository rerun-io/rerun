from __future__ import annotations

from typing import Any

import numpy.typing as npt


class QuaternionExt:
    def __init__(self: Any, *, xyzw: npt.ArrayLike) -> None:
        self.__attrs_init__(xyzw=xyzw)
