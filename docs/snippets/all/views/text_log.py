"""Use a blueprint to show a text log."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_text_log", spawn=True)

rr.set_time("time", sequence=0)
rr.log("log/status", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO))
rr.set_time("time", sequence=5)
rr.log("log/other", rr.TextLog("A warning.", level=rr.TextLogLevel.WARN))
for i in range(10):
    rr.set_time("time", sequence=i)
    rr.log("log/status", rr.TextLog(f"Processing item {i}.", level=rr.TextLogLevel.INFO))

# Create a text view that displays all logs.
blueprint = rrb.Blueprint(
    rrb.TextLogView(
        origin="/log",
        name="Text Logs",
        columns=rrb.TextLogColumns(
            timeline_columns=["time"],
            text_log_columns=["loglevel", "entitypath", "body"],
        ),
        rows=rrb.TextLogRows(
            filter_by_log_level=["INFO", "WARN", "ERROR"],
        ),
        format_options=rrb.TextLogFormat(
            monospace_body=False,
        ),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
