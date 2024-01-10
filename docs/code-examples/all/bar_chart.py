"""Create and log a bar chart."""

import rerun as rr

rr.init("rerun_example_bar_chart", spawn=True)
rr.log("bar_chart", rr.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))
