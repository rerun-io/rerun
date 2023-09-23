"""Create and log a bar chart."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_bar_chart", spawn=True)
rr2.log("bar_chart", rr2.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))
