import marimo

__generated_with = "0.25.0"
app = marimo.App(width="medium")


@app.cell
def _():
    import math
    import time
    import uuid

    import marimo as mo
    import numpy as np

    import rerun as rr  # pip install rerun-sdk
    import rerun.blueprint as rrb
    from rerun.notebook import Viewer  # pip install rerun-notebook
    from rerun.utilities import build_color_grid

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    return STEPS, Viewer, build_color_grid, mo, rr, rrb, time, twists, uuid


@app.cell
def _(mo):
    mo.md("""
    ## Logging some data

    Log some data to a recording, then show it with `notebook_show`.
    """)
    return


@app.cell
def _(STEPS, build_color_grid, rr, twists):
    rr.init("rerun_example_cube_marimo")

    for _t in range(STEPS):
        rr.set_time("step", sequence=_t)
        _cube = build_color_grid(10, 10, 10, twist=twists[_t])
        rr.log("cube", rr.Points3D(_cube.positions, colors=_cube.colors, radii=0.5))

    rr.notebook_show()
    return


@app.cell
def _(mo):
    mo.md("""
    ## Logging live data

    marimo delivers messages from the viewer only between cells, so a viewer cannot receive data until the cell that created it has finished.
    To stream live, create the viewer in one cell and log from a later one.
    """)
    return


@app.cell
def _(Viewer, rr, uuid):
    live_rec = rr.RecordingStream("rerun_example_cube_marimo_live", recording_id=uuid.uuid4())
    live_viewer = Viewer(recording=live_rec)
    live_viewer
    return (live_rec,)


@app.cell
def _(STEPS, build_color_grid, live_rec, rr, time, twists):
    for _t in range(STEPS):
        time.sleep(0.05)
        live_rec.set_time("step", sequence=_t)
        _cube = build_color_grid(10, 10, 10, twist=twists[_t])
        live_rec.log("cube", rr.Points3D(_cube.positions, colors=_cube.colors, radii=0.5))
    return


@app.cell
def _(mo):
    mo.md("""
    ## Using blueprints

    A blueprint as the last expression of a cell shows a viewer for the global recording.
    """)
    return


@app.cell
def _(STEPS, build_color_grid, rr, rrb, twists):
    rr.init("rerun_example_cube_marimo")

    for _t in range(STEPS):
        rr.set_time("step", sequence=_t)
        _h_grid = build_color_grid(10, 3, 3, twist=twists[_t])
        rr.log("h_grid", rr.Points3D(_h_grid.positions, colors=_h_grid.colors, radii=0.5))
        _v_grid = build_color_grid(3, 3, 10, twist=twists[_t])
        rr.log("v_grid", rr.Points3D(_v_grid.positions, colors=_v_grid.colors, radii=0.5))

    rrb.Blueprint(
        rrb.Horizontal(
            rrb.Spatial3DView(name="Horizontal grid", origin="h_grid"),
            rrb.Spatial3DView(name="Vertical grid", origin="v_grid"),
            column_shares=[2, 1],
        ),
        collapse_panels=True,
    )
    return


@app.cell
def _(mo):
    mo.md("""
    ## Controlling the viewer

    Hold on to a `Viewer` to add recordings to it and control it from Python.
    """)
    return


@app.cell
def _(STEPS, Viewer, build_color_grid, rr, twists):
    control_viewer = Viewer()

    for _rec_id, _color in [("example_a", [0, 255, 0]), ("example_b", [255, 0, 0])]:
        _rec = rr.RecordingStream("rerun_example_cube_marimo_time_ctrl", recording_id=_rec_id)
        control_viewer.add_recording(_rec)
        for _t in range(STEPS):
            _cube = build_color_grid(10, 10, 10, twist=twists[_t])
            _rec.set_time("step", sequence=_t)
            _rec.log("cube", rr.Points3D(_cube.positions, colors=_color, radii=0.5))

    control_viewer
    return (control_viewer,)


@app.cell
def _(control_viewer):
    control_viewer.update_panels(blueprint="expanded")
    control_viewer.set_active_recording(recording_id="example_a")
    control_viewer.set_time_ctrl(timeline="step", sequence=25)
    return


if __name__ == "__main__":
    app.run()
