"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

rec.set_properties({"started": 0, "name": "My recording (initial)"})

# Overwrites the name from above.
rec.set_name("My recording")
