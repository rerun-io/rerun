"""Log some text entries."""
import rerun as rr

rr.init("rerun_example_text_entry", spawn=True)

# TODO(#2793): TextLog archetype
# Log a direct entry directly
rr.log_text_entry("logs", "this entry has loglevel TRACE", level=rr.LogLevel.TRACE)
rr.log_text_entry("logs", "this other entry has loglevel INFO", level=rr.LogLevel.INFO)
