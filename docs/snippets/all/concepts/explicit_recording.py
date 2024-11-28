"""Just makes sure that explicit recordings actually work."""

import os

import rerun as rr

rec = rr.new_recording("rerun_example_explicit_recording")

rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]), recording=rec)

dir = os.path.dirname(os.path.abspath(__file__))
rr.log_file_from_path(os.path.join(dir, "../../../../tests/assets/cube.glb"), recording=rec)
