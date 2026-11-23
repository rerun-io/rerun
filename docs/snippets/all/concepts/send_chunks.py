"""Send chunks loaded from an RRD into a recording stream."""

import sys

import rerun as rr
from rerun.chunk import RrdReader

path_to_rrd = sys.argv[1]

# NOTE: This is specifically demonstrating how to forward chunks from an RRD
# into the viewer.
# If you just want to view an RRD file, use the simpler
# `rr.log_file_from_path()` function instead:
#   rr.init("rerun_example_app", spawn=True)
#   rr.log_file_from_path("path/to/file.rrd")

reader = RrdReader(path_to_rrd)
entry = reader.recordings()[0]

rr.init(entry.application_id, recording_id=entry.recording_id, spawn=True)
rr.send_chunks(reader.store())
