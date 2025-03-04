"""Send a dataframe to a new recording stream."""

import sys

import rerun as rr

path_to_rrd = sys.argv[1]

recording = rr.dataframe.load_recording(path_to_rrd)

rr.init(recording.application_id(), recording_id=recording.recording_id(), spawn=True)
rr.send_recording(recording)
