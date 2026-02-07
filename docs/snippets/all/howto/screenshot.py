"""Take screenshots of the viewer or specific views from code."""

import rerun as rr
import rerun.blueprint as rrb
from rerun.experimental import ViewerClient

# Setup a viewer with a known blueprint.
rr.init("rerun_example_screenshot", spawn=True)
view = rrb.Spatial3DView(name="my blue 3D", background=[100, 149, 237])
rr.send_blueprint(view)

# Connect to a local viewer.
viewer = ViewerClient()

# Screenshot the entire viewer.
viewer.save_screenshot("entire_viewer.jpg")

# Screenshot only the view we created earlier.
viewer.save_screenshot("my_view.png", view_id=view.id)
