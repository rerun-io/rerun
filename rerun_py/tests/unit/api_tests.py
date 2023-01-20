from rerun.log.text import LogLevel

import rerun as rr


def test_text():
    rr.log_text_entry("path", "text", level=None)
    rr.log_text_entry("path", "text", level=LogLevel.INFO)
    rr.log_text_entry("path", None, level=LogLevel.INFO)
