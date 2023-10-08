#!/usr/bin/env python3
"""Log a `TextLog`."""

import rerun as rr

rr.init("rerun_example_text_log", spawn=True)

rr.log("log", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO))
