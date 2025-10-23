"""Use a blueprint to show a bar chart."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_bar_chart", spawn=True)
rr.log("bar_chart", rr.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))

# Create a bar chart view to display the chart.
blueprint = rrb.Blueprint(
    rrb.BarChartView(
        origin="bar_chart",
        name="Bar Chart",
        background=rrb.archetypes.PlotBackground(color=[50, 0, 50, 255], show_grid=False),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
