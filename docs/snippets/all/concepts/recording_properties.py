"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

rec.set_properties(rr.RecordingProperties(name="My recording (initial)", start_time=0))

# Overwrites the name from above.
rec.set_name("My recording")
