#!/usr/bin/env python3
"""Log a `TextLog`."""

import rerun as rr

rr.init("rerun_example_text_log")
rr.save("/tmp/logs.rrd")

rr.log("log", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO))

for i in range(0, 100000):
    rr.log("log", rr.TextLog(f"Iteration #{i}", level=rr.TextLogLevel.INFO))
