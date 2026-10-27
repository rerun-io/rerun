"""Log a geospatial polygon with a hole."""

import rerun as rr
from rerun.components import GeoPolygon

rr.init("rerun_example_geo_polygons", spawn=True)
rr.log(
    "region",
    rr.GeoPolygons(
        [
            GeoPolygon(
                exterior=[
                    [48.85, 2.28],
                    [48.85, 2.36],
                    [48.90, 2.36],
                    [48.90, 2.28],
                ],
                holes=[
                    [[48.86, 2.30], [48.86, 2.33], [48.88, 2.33], [48.88, 2.30]]
                ],
            )
        ],
        colors=[[40, 120, 230, 128]],
    ),
)
