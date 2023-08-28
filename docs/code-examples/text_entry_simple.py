"""Log some text entries."""
import logging

import rerun as rr

rr.init("rerun_example_text_entry", spawn=True)

# Log a direct entry directly
rr.log_text_entry("logs", "this entry has loglevel TRACE", level=rr.LogLevel.TRACE)

# Or log via a logging handler
logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
logging.getLogger().setLevel(-1)
logging.info("This log got added through a `LoggingHandler`")
