#!/usr/bin/env python3

import rerun as rr

rr.init("rerun_example_native_sync")

# Open a local file handle to stream the data into.
rr.save("/tmp/my_recording.rrd")

# Log data as usual, thereby writing it into the file.
while True:
    rr.log("/", rr.TextLog("Logging things..."))
