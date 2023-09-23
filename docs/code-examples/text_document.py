#!/usr/bin/env python3
"""Log a `TextDocument`."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_text_document", spawn=True)

rr2.log("text_document", rr2.TextDocument(body="Hello, TextDocument!"))
rr2.log(
    "markdown",
    rr2.TextDocument(
        body="# Hello\nMarkdown with `code`!\n\nA random image:\n\n![A random image](https://picsum.photos/640/480)",
        media_type=rr2.cmp.MediaType.markdown(),
    ),
)
