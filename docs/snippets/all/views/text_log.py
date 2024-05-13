"""Use a blueprint to show a text log."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_text_log", spawn=True)

rr.set_time_sequence("time", 0)
rr.log("log/status", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO))
rr.set_time_sequence("time", 5)
rr.log("log/other", rr.TextLog("A warning.", level=rr.TextLogLevel.WARN))
rr.set_time_sequence("time", 10)
rr.log("log/status", rr.TextLog("Application ended.", level=rr.TextLogLevel.INFO))

# Create a text view that displays all logs.
blueprint = rrb.Blueprint(rrb.TextLogView(origin="/log", name="Text Logs"))

rr.send_blueprint(blueprint)
