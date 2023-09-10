#!/usr/bin/env python3
"""Log a `TextDocument`."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_text_document", spawn=True)

rr2.log("text_document", rr2.TextDocument(body="Hello, TextDocument!"))
