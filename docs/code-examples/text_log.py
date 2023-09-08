#!/usr/bin/env python3
"""Log a `TextLog`."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_text_log", spawn=True)

# TODO(emilk): show how to hook up to the log stream.
rr2.log("log", rr2.TextLog(body="Application started.", level="INFO"))
