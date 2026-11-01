from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

if TYPE_CHECKING:
    from collections.abc import Iterable

    import numpy.typing as npt


class GeoPolygonExt:
    """Extension for [GeoPolygon][rerun.components.GeoPolygon]."""

    def __init__(
        self: Any,
        *,
        exterior: npt.ArrayLike,
        holes: Iterable[npt.ArrayLike] = (),
    ) -> None:
        """
        Create a polygon from latitude/longitude rings, with optional holes.

        Each ring must contain at least three finite pairs of latitude and longitude.
        Either winding order is accepted, and the last vertex need not repeat the first.
        """
        rings = [np.asarray(ring, dtype=np.float64) for ring in (exterior, *holes)]
        for ring in rings:
            if ring.ndim != 2 or ring.shape[1] != 2 or len(ring) < 3:
                raise ValueError("Each polygon ring must have shape (N, 2), with N >= 3.")
            if not np.isfinite(ring).all():
                raise ValueError("Polygon coordinates must be finite.")
        self.__attrs_init__(
            lat_lon=np.concatenate(rings),
            ring_lengths=np.asarray([len(ring) for ring in rings], dtype=np.uint64),
        )
