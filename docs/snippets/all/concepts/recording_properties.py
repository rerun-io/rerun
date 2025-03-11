"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

rec.set_recording_name("My Recording")
