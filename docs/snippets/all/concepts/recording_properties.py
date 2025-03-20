"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

rec.set_properties(rr.archetypes.RecordingProperties(name="My recording (initial)", start_time=0))

# Overwrites the name from above.
rec.set_recording_name("My recording")

# Overwrites the start time from above.
rec.set_recording_start_time_nanos(42)

rec.set_properties(
    rr.archetypes.Points3D([1.0, 0.1, 1.0]),
    entity_path="cameras/left",
)
