#!/usr/bin/env python3
"""Log a `TextLog`."""

import time

import rerun as rr

rr.init("rerun_example_text_log")
rr.save("/tmp/kek.rrd")

for i in range(0, 1000):
    rr.set_time_sequence("frame", i)
    rr.log("log", rr.TextLog(f"log {i}", level=rr.TextLogLevel.INFO))
    time.sleep(2.0)
