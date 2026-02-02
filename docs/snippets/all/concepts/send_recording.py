"""Send a dataframe to a new recording stream."""

import sys

import rerun as rr

path_to_rrd = sys.argv[1]

# NOTE: This is specifically demonstrating how to send `rr.recording.Recording` into the viewer.
# If you just want to view an RRD file, use the simpler `rr.log_file()` function instead:
#   rr.log_file("path/to/file.rrd", spawn=True)

recording = rr.recording.load_recording(path_to_rrd)

rr.init(recording.application_id(), recording_id=recording.recording_id(), spawn=True)
rr.send_recording(recording)
