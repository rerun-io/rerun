"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

# Overwrites the name from above.
rec.send_recording_name("My recording")

# Overwrites the start time from above.
rec.send_recording_start_time_nanos(42)

rec.send_property(
    "camera_left",
    rr.archetypes.Points3D([1.0, 0.1, 1.0]),
)
