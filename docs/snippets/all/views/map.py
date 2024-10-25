"""Use a blueprint to show a map."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_map_view", spawn=True)

rr.log("points", rr.GeoPoints([[47.6344, 19.1397], [47.6334, 19.1399]]))

# Create a map view to display the chart.
# TODO(#7903): cleanup the blueprint API for the map view
blueprint = rrb.Blueprint(
    rrb.MapView(
        origin="points",
        name="MapView",
        zoom=rrb.archetypes.MapZoom(16.0),
        background=rrb.archetypes.MapBackground(rrb.components.MapProvider.OpenStreetMap),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
