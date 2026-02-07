"""Create and set a file sink."""

import rerun as rr

rr.init("rerun_example_file_sink")

rr.set_sinks(rr.FileSink("recording.rrd"))
