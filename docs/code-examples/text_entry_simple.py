"""Log a text entries."""
import logging

import rerun as rr

rr.init("text_entry", spawn=True)

rr.log_text_entry("logs", "this entry has loglevel TRACE", level=rr.LogLevel.TRACE)

logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
logging.getLogger().setLevel(-1)
logging.info("This log got added through a `LoggingHandler`")
