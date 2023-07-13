from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import Angle


def angle_init(self: Angle, rad: float | None = None, deg: float | None = None) -> None:
    if rad is not None:
        self.__attrs_init__(inner=rad, kind="radians")  # pyright: ignore[reportGeneralTypeIssues]
    elif deg is not None:
        self.__attrs_init__(inner=deg, kind="degrees")  # pyright: ignore[reportGeneralTypeIssues]
    else:
        raise ValueError("Either `rad` or `deg` must be provided.")
