"""Sets the recording properties."""

import rerun as rr

rec = rr.RecordingStream("rerun_example_recording_properties")
rec.spawn()

# Overwrites the name from above.
rec.send_recording_name("My recording")

# Start time is set automatically, but we can overwrite it at any time.
rec.send_recording_start_time_nanos(1742539110661000000)

# Adds a user-defined property to the recording.
rec.send_property(
    "camera_left",
    rr.archetypes.Points3D([[1.0, 0.1, 1.0]]),
)

# Adds another property, this time with user-defined data.
rec.send_property(
    "situation",
    rr.AnyValues(
        confidences=[0.3, 0.4, 0.5, 0.6],
        traffic="low",
        weather="sunny",
    ),
)

# Properties, including the name, can be overwritten at any time.
rec.send_recording_name("My episode")
