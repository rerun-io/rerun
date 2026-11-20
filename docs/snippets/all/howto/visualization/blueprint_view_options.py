"""Configure containers, view properties, and visible time ranges."""

import math

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_blueprint_view_options", spawn=True)

for step in range(200):
    t = step * 0.1
    rr.set_time("time", duration=t)
    rr.log(
        "world/robot/trajectory",
        rr.Points3D([[math.cos(t), math.sin(t), 0.05 * t]]),
    )
    rr.log("sensors/temperature", rr.Scalars(20.0 + math.sin(t)))
    rr.log("metrics/speed", rr.Scalars(0.5 + 0.1 * math.cos(t)))

rr.log(
    "diagnostics", rr.TextLog("Gripper stalled", level=rr.TextLogLevel.ERROR)
)

robot_error = True

# region: send_on_error
if robot_error:
    # Show diagnostic views for debugging
    blueprint = rrb.Grid(
        rrb.Spatial3DView(name="Robot view", origin="/world/robot"),
        rrb.TextLogView(name="Error Logs", origin="/diagnostics"),
        rrb.TimeSeriesView(name="Sensor Data", origin="/sensors"),
    )
    rr.send_blueprint(blueprint, make_active=True)
# endregion: send_on_error

# region: tabs
tabs = rrb.Blueprint(
    rrb.Tabs(
        rrb.Spatial3DView(name="Scene", origin="/world"),
        rrb.TimeSeriesView(name="Plots", origin="/metrics"),
        active_tab="Plots",
    )
)
# endregion: tabs
rr.send_blueprint(tabs)

# region: spatial3d_properties
# Configure a 3D view with custom camera settings
robot_view = rrb.Spatial3DView(
    name="Robot view",
    origin="/world/robot",
    background=[100, 149, 237],  # Light blue
    eye_controls=rrb.EyeControls3D(
        kind=rrb.Eye3DKind.FirstPerson,
        speed=20.0,
    ),
)
# endregion: spatial3d_properties

# region: time_series_properties
# Configure a time series view with custom axis and time ranges
sensor_view = rrb.TimeSeriesView(
    name="Sensor Data",
    origin="/sensors",
    axis_y=rrb.ScalarAxis(range=(-10.0, 10.0), zoom_lock=True),
    plot_legend=rrb.PlotLegend(visible=False),
    time_ranges=[
        rrb.VisibleTimeRange(
            "time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    ],
)
# endregion: time_series_properties

# region: per_entity_time_range
trajectory_view = rrb.Spatial3DView(
    origin="/",
    overrides={
        "/world/robot/trajectory": rrb.VisibleTimeRanges(
            timeline="time",
            start=rrb.TimeRangeBoundary.infinite(),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    },
)
# endregion: per_entity_time_range

# region: scrolling_plot
scrolling_plot = rrb.TimeSeriesView(
    origin="/sensors",
    time_ranges=rrb.VisibleTimeRange(
        "time",
        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
        end=rrb.TimeRangeBoundary.cursor_relative(),
    ),
    axis_x=rrb.TimeAxis(
        view_range=rrb.TimeRange(
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
        zoom_lock=True,
    ),
)
# endregion: scrolling_plot

# region: fixed_window
fixed_window_plot = rrb.TimeSeriesView(
    origin="/sensors",
    time_ranges=rrb.VisibleTimeRange(
        timeline="time",
        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
        end=rrb.TimeRangeBoundary.cursor_relative(),
    ),
)
# endregion: fixed_window

# region: axis_and_legend
axis_plot = rrb.TimeSeriesView(
    origin="/sensors",
    axis_y=rrb.ScalarAxis(range=(-1.0, 1.0), zoom_lock=True),
    plot_legend=rrb.PlotLegend(visible=False),
)
# endregion: axis_and_legend

rr.send_blueprint(
    rrb.Blueprint(
        rrb.Tabs(
            robot_view,
            sensor_view,
            trajectory_view,
            scrolling_plot,
            fixed_window_plot,
            axis_plot,
        ),
    )
)
