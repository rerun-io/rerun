"""Shows integration of Rerun's `TextLog` with the native logging interface."""
import logging

import rerun as rr

rr.init("rerun_example_text_log_integration", spawn=True)

# Log a text entry directly
rr.log("logs", rr.TextLog("this entry has loglevel TRACE", level=rr.TextLogLevel.TRACE))

# Or log via a logging handler
logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
logging.getLogger().setLevel(-1)
logging.info("This INFO log got added through the standard logging interface")
