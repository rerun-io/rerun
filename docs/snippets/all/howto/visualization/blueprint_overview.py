"""Send a series of blueprints that combine many kinds of views in a grid."""

import math

import numpy as np
from numpy.random import default_rng

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_blueprint_overview", spawn=True)

rr.set_time("time", sequence=0)
rr.log(
    "log/status", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO)
)
rr.set_time("time", sequence=5)
rr.log("log/other", rr.TextLog("A warning.", level=rr.TextLogLevel.WARN))
for i in range(10):
    rr.set_time("time", sequence=i)
    rr.log(
        "log/status",
        rr.TextLog(f"Processing item {i}.", level=rr.TextLogLevel.INFO),
    )

# Create a text view that displays all logs.
blueprint = rrb.Blueprint(
    rrb.TextLogView(origin="/log", name="Text Logs"),
    rrb.SelectionPanel(state="expanded"),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)

# Create a spiral of points:
n = 150
angle = np.linspace(0, 10 * np.pi, n)
spiral_radius = np.linspace(0.0, 3.0, n) ** 2
positions = np.column_stack((
    np.cos(angle) * spiral_radius,
    np.sin(angle) * spiral_radius,
))
colors = np.dstack((
    np.linspace(255, 255, n),
    np.linspace(255, 0, n),
    np.linspace(0, 255, n),
))[0].astype(int)
radii = np.linspace(0.01, 0.7, n)

rr.log("points", rr.Points2D(positions, colors=colors, radii=radii))

# Create a Spatial2D view to display the points.
blueprint = rrb.Blueprint(
    rrb.Spatial2DView(
        origin="/",
        name="2D Scene",
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)

rr.log(
    "points",
    rr.GeoPoints(
        lat_lon=[[47.6344, 19.1397], [47.6334, 19.1399]],
        radii=rr.Radius.ui_points(20.0),
    ),
)

# Create a map view to display the chart.
blueprint = rrb.Blueprint(
    rrb.MapView(
        origin="points",
        name="MapView",
        zoom=16.0,
        background=rrb.MapProvider.OpenStreetMap,
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)

blueprint = rrb.Blueprint(
    rrb.Grid(
        rrb.MapView(
            origin="points",
            name="MapView",
            zoom=16.0,
            background=rrb.MapProvider.OpenStreetMap,
        ),
        rrb.Spatial2DView(
            origin="/",
            name="2D Scene",
        ),
        rrb.TextLogView(origin="/log", name="Text Logs"),
    ),
    rrb.TimePanel(state="expanded"),
    rrb.BlueprintPanel(state="expanded"),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)

rr.log("bar_chart", rr.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))

rng = default_rng(12345)
positions = rng.uniform(-5, 5, size=[50, 3])
colors = rng.uniform(0, 255, size=[50, 3])
radii = rng.uniform(0.1, 0.5, size=[50])

rr.log("3dpoints", rr.Points3D(positions, colors=colors, radii=radii))

tensor = rng.integers(0, 256, (32, 60, 80, 3), dtype=np.uint8)
rr.log("tensor", rr.Tensor(tensor, dim_names=("batch", "x", "y", "channel")))

rr.log(
    "markdown",
    rr.TextDocument(
        """
# Hello Markdown!
[Click here to see the raw text](recording://markdown:Text).
"""
    ),
)

rr.log(
    "trig/sin",
    rr.SeriesLines(colors=[255, 0, 0], names="sin(0.01t)"),
    static=True,
)
for t in range(int(math.pi * 4 * 100.0)):
    rr.set_time("time", sequence=t)
    rr.set_time("timeline1", duration=t)
    rr.log("trig/sin", rr.Scalars(math.sin(float(t) / 100.0)))

blueprint = rrb.Blueprint(
    rrb.Grid(
        rrb.MapView(
            origin="points",
            name="MapView",
            zoom=16.0,
            background=rrb.MapProvider.OpenStreetMap,
        ),
        rrb.Spatial2DView(
            origin="/",
            name="2D Scene",
        ),
        rrb.TextLogView(origin="/log", name="Text Logs"),
        rrb.BarChartView(origin="bar_chart", name="Bar Chart"),
        rrb.Spatial3DView(
            origin="/3dpoints",
            name="3D Scene",
            # Set the background color to light blue.
            background=[100, 149, 237],
            # Configure the eye controls.
            eye_controls=rrb.EyeControls3D(
                kind=rrb.Eye3DKind.FirstPerson,
                speed=20.0,
            ),
        ),
        rrb.TensorView(
            origin="tensor",
            name="Tensor",
        ),
        rrb.TextDocumentView(origin="markdown", name="Markdown example"),
        rrb.TimeSeriesView(
            origin="/trig",
            # Set a custom Y axis.
            axis_y=rrb.ScalarAxis(range=(-1.0, 1.0), zoom_lock=True),
            # Configure the legend.
            plot_legend=rrb.PlotLegend(visible=False),
            # Set different time ranges for different timelines.
            time_ranges=[
                # Sliding window around the time cursor on the first timeline.
                rrb.VisibleTimeRange(
                    "time",
                    start=rrb.TimeRangeBoundary.cursor_relative(seq=-100),
                    end=rrb.TimeRangeBoundary.cursor_relative(),
                ),
                # From a fixed time to the end of the second timeline.
                rrb.VisibleTimeRange(
                    "timeline1",
                    start=rrb.TimeRangeBoundary.absolute(seconds=300.0),
                    end=rrb.TimeRangeBoundary.infinite(),
                ),
            ],
        ),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
