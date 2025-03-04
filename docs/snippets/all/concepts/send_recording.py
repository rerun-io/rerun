"""Send a dataframe to a new recording stream."""

import sys

import rerun as rr

path_to_rrd = sys.argv[1]

recording = rr.dataframe.load_recording(path_to_rrd)

rr.init("rerun_example_send_recording")
rr.send_recording(recording)
