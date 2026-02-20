"""Just makes sure that explicit recordings actually work."""

import os

import rerun as rr

rec = rr.RecordingStream("rerun_example_explicit_recording")

rec.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))

dir = os.path.dirname(os.path.abspath(__file__))
rec.log_file_from_path(os.path.join(dir, "../../../../tests/assets/mesh/cube.glb"))
