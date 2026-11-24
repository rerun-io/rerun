"""Log to two recordings from one process."""

import uuid

import rerun as rr

rec_a = rr.RecordingStream(
    "rerun_example_multiple_recordings", recording_id=uuid.uuid4()
)
rec_b = rr.RecordingStream(
    "rerun_example_multiple_recordings", recording_id=uuid.uuid4()
)

rec_a.spawn()
rec_b.connect_grpc()  # The Viewer started by `rec_a.spawn()` is now running.

rec_a.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
rec_b.log("points", rr.Points3D([[2, 2, 2], [3, 3, 3]]))
