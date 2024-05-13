"""Use a blueprint to show a bar chart."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_bar_chart", spawn=True)
# It's recommended to log bar charts with the `rr.BarChart` archetype,
# but single dimensional tensors can also be used if a `BarChartView` is created explicitly.
rr.log("tensor", rr.Tensor([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))

# Create a bar chart view to display the chart.
blueprint = rrb.Blueprint(rrb.BarChartView(origin="tensor", name="Bar Chart"), collapse_panels=True)

rr.send_blueprint(blueprint)
