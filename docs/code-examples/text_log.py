#!/usr/bin/env python3
"""Log a `TextLog`."""

import rerun as rr

rr.init("rerun_example_text_log", spawn=True)

# TODO(emilk): show how to hook up to the log stream.
rr.log("log", rr.TextLog("Application started.", level="INFO"))
