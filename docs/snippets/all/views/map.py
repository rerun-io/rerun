"""Use a blueprint to customize a map view."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_map_view", spawn=True)

rr.log("points", rr.GeoPoints(lat_lon=[[47.6344, 19.1397], [47.6334, 19.1399]], radii=rr.Radius.ui_points(20.0)))

# Create a map view to display the chart.
blueprint = rrb.Blueprint(
    rrb.MapView(
        origin="points",
        name="MapView",
        zoom=16.0,
        background=rrb.MapProvider.OpenStreetMap,
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
