"""Log some very simple geospatial point."""

import rerun as rr

rr.init("rerun_example_geo_points", spawn=True)

rr.log(
    "rerun_hq",
    rr.GeoPoints(
        lat_lon=[59.319221, 18.075631],
        radii=rr.Radius.ui_points(10.0),
        colors=[255, 0, 0],
    ),
)
