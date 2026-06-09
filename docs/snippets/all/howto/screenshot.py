"""Take screenshots of the viewer or specific views from code."""

import rerun as rr
import rerun.blueprint as rrb
from rerun.experimental import ViewerClient

# Spawn a headless viewer; the client owns its lifetime.
with ViewerClient.spawn(headless=True) as viewer:
    rec = rr.RecordingStream("rerun_example_screenshot")
    rec.connect_grpc(url=viewer.url)

    view = rrb.Spatial3DView(name="my blue 3D", background=[100, 149, 237])
    rec.send_blueprint(view)

    # Screenshot the entire viewer.
    viewer.save_screenshot("entire_viewer.jpg")

    # Screenshot only the view we created earlier.
    viewer.save_screenshot("my_view.png", view_id=view.id)

    # Disconnect the RecordingStream before the headless viewer shuts down.
    rec.disconnect()
