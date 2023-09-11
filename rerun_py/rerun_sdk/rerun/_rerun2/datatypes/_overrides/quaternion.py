from __future__ import annotations

from typing import TYPE_CHECKING

import numpy.typing as npt

if TYPE_CHECKING:
    from .. import Quaternion


def override_quaternion_init_override(self: Quaternion, *, xyzw: npt.ArrayLike) -> None:
    self.__attrs_init__(xyzw=xyzw)
