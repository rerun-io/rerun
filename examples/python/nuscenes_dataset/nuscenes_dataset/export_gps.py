# from nuScenes dev-kit: https://github.com/nutonomy/nuscenes-devkit/blob/4df2701feb3436ae49edaf70128488865a3f6ff9/python-sdk/nuscenes/scripts/export_poses.py
# Code contributed by jean-lucas, 2020.

"""Exports the nuScenes ego poses as "GPS" coordinates (lat/lon) for each scene into JSON or KML formatted files."""

from __future__ import annotations

import math
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence

EARTH_RADIUS_METERS = 6.378137e6
REFERENCE_COORDINATES = {
    "boston-seaport": [42.336849169438615, -71.05785369873047],
    "singapore-onenorth": [1.2882100868743724, 103.78475189208984],
    "singapore-hollandvillage": [1.2993652317780957, 103.78217697143555],
    "singapore-queenstown": [1.2782562240223188, 103.76741409301758],
}


def get_coordinate(ref_lat: float, ref_lon: float, bearing: float, dist: float) -> tuple[float, float]:
    """
    Using a reference coordinate, extract the coordinates of another point in space given its distance and bearing
    to the reference coordinate. For reference, please see: https://www.movable-type.co.uk/scripts/latlong.html.

    Parameters
    ----------
    ref_lat : float
        Latitude of the reference coordinate in degrees, e.g., 42.3368.
    ref_lon : float
        Longitude of the reference coordinate in degrees, e.g., 71.0578.
    bearing : float
        The clockwise angle in radians between the target point, reference point, and the axis pointing north.
    dist : float
        The distance in meters from the reference point to the target point.

    Returns
    -------
    tuple[float, float]
        A tuple of latitude and longitude.

    """  # noqa: D205
    lat, lon = math.radians(ref_lat), math.radians(ref_lon)
    angular_distance = dist / EARTH_RADIUS_METERS

    target_lat = math.asin(
        math.sin(lat) * math.cos(angular_distance) + math.cos(lat) * math.sin(angular_distance) * math.cos(bearing),
    )
    target_lon = lon + math.atan2(
        math.sin(bearing) * math.sin(angular_distance) * math.cos(lat),
        math.cos(angular_distance) - math.sin(lat) * math.sin(target_lat),
    )
    return math.degrees(target_lat), math.degrees(target_lon)


def derive_latlon(location: str, pose: dict[str, Sequence[float]]) -> tuple[float, float]:
    """
    Extract lat/lon coordinate from pose.

    This makes the following two assumptions in order to work:
        1. The reference coordinate for each map is in the south-western corner.
        2. The origin of the global poses is also in the south-western corner (and identical to 1).

    Parameters
    ----------
    location : str
        The name of the map the poses correspond to, i.e., `boston-seaport`.
    pose : dict[str, Sequence[float]]
        nuScenes egopose.

    Returns
    -------
    tuple[float, float]
    Latitude and longitude coordinates in degrees.

    """
    assert location in REFERENCE_COORDINATES.keys(), (
        f"Error: The given location: {location}, has no available reference."
    )

    reference_lat, reference_lon = REFERENCE_COORDINATES[location]
    x, y = pose["translation"][:2]
    bearing = math.atan(x / y)
    distance = math.sqrt(x**2 + y**2)
    lat, lon = get_coordinate(reference_lat, reference_lon, bearing, distance)

    return lat, lon
