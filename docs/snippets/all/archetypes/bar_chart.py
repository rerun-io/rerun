"""Create and log a bar chart."""

import rerun as rr

rr.init("rerun_example_bar_chart", spawn=True)
rr.log("bar_chart", rr.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))
rr.log("bar_chart_custom_abscissa", rr.BarChart([8, 4, 0, 9, 1, 4], abscissa=[0, 1, 3, 4, 7, 11]))
rr.log(
    "bar_chart_custom_abscissa_and_widths",
    rr.BarChart([8, 4, 0, 9, 1, 4], abscissa=[0, 1, 3, 4, 7, 11], widths=[1, 2, 1, 3, 4, 1]),
)
