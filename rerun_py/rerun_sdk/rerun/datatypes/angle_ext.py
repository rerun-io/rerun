from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass


class AngleExt:
    """Extension for [Angle][rerun.datatypes.Angle]."""

    def __init__(self: Any, rad: float | None = None, deg: float | None = None) -> None:
        """
        Create a new instance of the Angle datatype.

        Parameters
        ----------
        rad:
            Angle in radians, specify either `rad` or `deg`.
        deg:
            Angle in degrees, specify either `rad` or `deg`.

        """

        if rad is not None:
            self.__attrs_init__(inner=rad, kind="radians")  # pyright: ignore[reportGeneralTypeIssues]
        elif deg is not None:
            self.__attrs_init__(inner=deg, kind="degrees")  # pyright: ignore[reportGeneralTypeIssues]
        else:
            raise ValueError("Either `rad` or `deg` must be provided.")
