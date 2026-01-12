import rerun as rr
import rerun.blueprint as rrb
from rerun.experimental import ViewerClient

# # Create a 3D view and memorize its ID
# view = rrb.Spatial3DView(name="Ma 3d view!", background=[100, 149, 237])
# rr.send_blueprint(rrb.Blueprint(view, collapse_panels=True))

# Connect to the viewer and request a screenshot of the specific view
viewer = ViewerClient("127.0.0.1:9876")
viewer.save_screenshot("demo/1.png")
