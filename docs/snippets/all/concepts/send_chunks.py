"""Send chunks loaded from an RRD into a recording stream."""

import sys

import rerun as rr
import rerun.experimental as rrx

path_to_rrd = sys.argv[1]

# NOTE: This is specifically demonstrating how to forward chunks from an RRD into the viewer.
# If you just want to view an RRD file, use the simpler `rr.log_file()` function instead:
#   rr.log_file("path/to/file.rrd", spawn=True)

reader = rrx.RrdReader(path_to_rrd)
entry = reader.recordings()[0]

rr.init(entry.application_id, recording_id=entry.recording_id, spawn=True)
rrx.send_chunks(reader.store())
