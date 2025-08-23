# noqa: I002
# We should actually clean this up but POC for now
# mypy: ignore-errors
# I think marimo clears this when we run things
# from __future__ import annotations

import marimo

__generated_with = "0.15.0"
app = marimo.App(width="medium")


@app.cell
def _(mo):
    mo.md(r"""## Rerun imports and initialization""")
    return


@app.cell
def _():
    from __future__ import annotations  # noqa: F404

    import math
    import uuid
    from collections import namedtuple
    from math import cos, sin

    import numpy as np
    import rerun as rr  # pip install rerun-sdk
    import rerun.blueprint as rrb
    from rerun.notebook import Viewer  # pip install rerun-notebook

    return Viewer, cos, math, namedtuple, np, rr, rrb, sin, uuid


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Helper to create the colored cube

    This code exists in the `rerun.utilities` package, but is included here for context.
    """
    )
    return


@app.cell
def _(cos, namedtuple, np, sin):
    ColorGrid = namedtuple("ColorGrid", ["positions", "colors"])

    def build_color_grid(x_count: int = 10, y_count: int = 10, z_count: int = 10, twist: float = 0) -> ColorGrid:
        """
        Create a cube of points with colors.

        The total point cloud will have x_count * y_count * z_count points.

        Parameters
        ----------
        x_count, y_count, z_count:
            Number of points in each dimension.
        twist:
            Angle to twist from bottom to top of the cube

        """

        grid = np.mgrid[
            slice(-x_count, x_count, x_count * 1j),
            slice(-y_count, y_count, y_count * 1j),
            slice(-z_count, z_count, z_count * 1j),
        ]

        angle = np.linspace(-float(twist) / 2, float(twist) / 2, z_count)
        for z in range(z_count):
            xv, yv, zv = grid[:, :, :, z]
            rot_xv = xv * cos(angle[z]) - yv * sin(angle[z])
            rot_yv = xv * sin(angle[z]) + yv * cos(angle[z])
            grid[:, :, :, z] = [rot_xv, rot_yv, zv]

        positions = np.vstack([xyz.ravel() for xyz in grid])

        colors = np.vstack([
            xyz.ravel()
            for xyz in np.mgrid[
                slice(0, 255, x_count * 1j),
                slice(0, 255, y_count * 1j),
                slice(0, 255, z_count * 1j),
            ]
        ])

        return ColorGrid(positions.T, colors.T.astype(np.uint8))

    return (build_color_grid,)


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Logging some data

    Now we can log some data and add it to the recording, and display it using `notebook_show`.

    Note that displaying a recording will consume the data, so it will not be available for use in a subsequent cell.
    """
    )
    return


@app.cell
def _(build_color_grid, math, np, rr):
    rr.init("rerun_example_cube")

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    for t in range(STEPS):
        rr.set_time("step", sequence=t)
        cube = build_color_grid(10, 10, 10, twist=twists[t])
        rr.log("cube", rr.Points3D(cube.positions, colors=cube.colors, radii=0.5))

    rr.notebook_show()
    return STEPS, cube, t


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Logging live data

    Using `rr.notebook_show` like above buffers the data in the recording stream, but doesn't process it until the call to `rr.notebook_show`.

    However, `rr.notebook_show` can be called at any time during your cell's execution to immediately display the Rerun viewer. You can then incrementally stream to it. Here we add a sleep to simulate a cell that does a lot more processing. By calling `notebook_show` first, we can see the output of our code live while it's still running.
    """
    )
    return


@app.cell
def _(build_color_grid, cube, math, np, rr, t):
    from time import sleep

    rr.init("rerun_example_cube")

    # rr.notebook_show()

    _STEPS = 100
    _twists = math.pi * np.sin(np.linspace(0, math.tau, _STEPS)) / 4
    for _t in range(_STEPS):
        sleep(0.05)
        rr.set_time("step", sequence=_t)
        _cube = build_color_grid(10, 10, 10, twist=_twists[t])
        rr.log("cube", rr.Points3D(_cube.positions, colors=cube.colors, radii=0.5))
    rr.notebook_show()
    return (sleep,)


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Incremental logging

    Note that until we either reset the recording stream (by calling `rr.init()`), or create a new output widget (by calling `rr.notebook_show()` The currently active stream in the kernel will continue to send events to the existing widget.

    The following will add a rotation to the above recording.
    """
    )
    return


@app.cell
def _(STEPS, rr, sleep, t):
    for _t in range(STEPS):
        sleep(0.01)
        rr.set_time("step", sequence=_t)
        rr.log("cube", rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[1, 0, 0], degrees=t)))
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Starting a new recording

    You can always start another recording by calling `rr.init(...)` again to reset the global stream, or alternatively creating a separate recording stream using the `rr.RecordingStream` constructor (discussed more below)
    """
    )
    return


@app.cell
def _(build_color_grid, math, np, rr, t):
    rr.init("rerun_example_cube")

    _STEPS = 100
    _twists = math.pi * np.sin(np.linspace(0, math.tau, _STEPS)) / 4
    for _t in range(_STEPS):
        rr.set_time("step", sequence=t)
        h_grid = build_color_grid(10, 3, 3, twist=_twists[_t])
        rr.log("h_grid", rr.Points3D(h_grid.positions, colors=h_grid.colors, radii=0.5))
        v_grid = build_color_grid(3, 3, 10, twist=_twists[_t])
        rr.log("v_grid", rr.Points3D(v_grid.positions, colors=v_grid.colors, radii=0.5))

    rr.notebook_show()
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Adjusting the view

    The  `notebook_show` method also lets you adjust properties such as width and height.
    """
    )
    return


@app.cell
def _(build_color_grid, rr):
    rr.init("rerun_example_cube")

    small_cube = build_color_grid(3, 3, 3, twist=0)
    rr.log("small_cube", rr.Points3D(small_cube.positions, colors=small_cube.colors, radii=0.5))

    rr.notebook_show(width="auto", height=400)
    return (small_cube,)


@app.cell
def _(mo):
    mo.md(
        r"""
    To update the default width and height, you can use the `rerun.notebook.set_default_size` function.

    Note that you do not need to initialize a recording to use this function.
    """
    )
    return


@app.cell
def _(build_color_grid, rr, small_cube):
    from rerun.notebook import set_default_size

    set_default_size(width=400, height=400)

    rr.init("rerun_example_cube")

    _small_cube = build_color_grid(3, 3, 3, twist=0)
    rr.log("small_cube", rr.Points3D(_small_cube.positions, colors=small_cube.colors, radii=0.5))

    rr.notebook_show()
    return (set_default_size,)


@app.cell
def _(set_default_size):
    set_default_size(width=640, height=480)
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Using blueprints

    Rerun blueprints can be used with `rr.notebook_show()`

    For example, we can split the two grids into their own respective views.
    """
    )
    return


@app.cell
def _(build_color_grid, math, np, rr, rrb):
    rr.init("rerun_example_cube")

    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Spatial3DView(name="Horizontal grid", origin="h_grid"),
            rrb.Spatial3DView(name="Vertical grid", origin="v_grid"),
            column_shares=[2, 1],
        ),
        collapse_panels=True,
    )

    rr.notebook_show(blueprint=blueprint)

    _STEPS = 100
    _twists = math.pi * np.sin(np.linspace(0, math.tau, _STEPS)) / 4
    for _t in range(_STEPS):
        rr.set_time("step", sequence=_t)
        _h_grid = build_color_grid(10, 3, 3, twist=_twists[_t])
        rr.log("h_grid", rr.Points3D(_h_grid.positions, colors=_h_grid.colors, radii=0.5))
        _v_grid = build_color_grid(3, 3, 10, twist=_twists[_t])
        rr.log("v_grid", rr.Points3D(_v_grid.positions, colors=_v_grid.colors, radii=0.5))
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Extra convenience

    Rerun blueprints types also implement `_ipython_display_()` directly, so if a blueprint is the last element in your cell the right thing will happen.

    Note that this mechanism only works when you are using the global recording stream.
    """
    )
    return


@app.cell
def _(build_color_grid, math, np, rr, rrb):
    def _():
        rr.init("rerun_example_cube")

        STEPS = 100
        twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
        for t in range(STEPS):
            rr.set_time("step", sequence=t)
            h_grid = build_color_grid(10, 3, 3, twist=twists[t])
            rr.log("h_grid", rr.Points3D(h_grid.positions, colors=h_grid.colors, radii=0.5))
            v_grid = build_color_grid(3, 3, 10, twist=twists[t])
            rr.log("v_grid", rr.Points3D(v_grid.positions, colors=v_grid.colors, radii=0.5))
        return rrb.Spatial3DView(name="Horizontal grid", origin="h_grid")

    _()
    rr.notebook_show()
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Working with non-global streams

    Sometimes it can be more explicit to work with specific (non-global recording) streams via `rr.RecordingStream` constructor.

    In this case, remember to call `notebook_show` directly on the recording stream. As noted above, there is no way to use a bare Blueprint object in conjunction with a non-global recording.
    """
    )
    return


@app.cell
def _(build_color_grid, rr, rrb):
    rec = rr.RecordingStream("rerun_example_cube_flat")

    bp = rrb.Blueprint(collapse_panels=True)

    rec.notebook_show(blueprint=bp)

    flat_grid = build_color_grid(20, 20, 1, twist=0)
    rec.log("flat_grid", rr.Points3D(flat_grid.positions, colors=flat_grid.colors, radii=0.5))
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Using the Viewer object directly

    Instead of calling `notebook_show` you can alternatively hold onto the viewer object.

    This lets you add additional recordings to a view.
    """
    )
    return


@app.cell
def _(Viewer, build_color_grid, rr, rrb, uuid):
    def _():
        rec = rr.RecordingStream("rerun_example_multi_recording", recording_id=uuid.uuid4())

        flat_grid = build_color_grid(20, 20, 1, twist=0)
        rec.log("flat_grid", rr.Points3D(flat_grid.positions, colors=flat_grid.colors, radii=0.5))

        viewer = Viewer(recording=rec.to_native(), blueprint=rrb.Blueprint(rrb.BlueprintPanel(state="expanded")))
        return viewer

    a_viewer = _()
    a_viewer.display()
    return (a_viewer,)


@app.cell
def _(a_viewer, build_color_grid, math, np, rr, uuid):
    def _(viewer):
        rec = rr.RecordingStream("rerun_example_multi_recording", recording_id=uuid.uuid4())

        viewer.add_recording(rec)

        STEPS = 100
        twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
        for t in range(STEPS):
            rec.set_time("step", sequence=t)
            cube = build_color_grid(10, 10, 10, twist=twists[t])
        return rec.log("cube", rr.Points3D(cube.positions, colors=cube.colors, radii=0.5))

    _(a_viewer)
    return


@app.cell
def _(mo):
    mo.md(
        r"""
    ## Controlling the Viewer

    Other than sending a blueprint to the Viewer, some parts of it can also be controlled directly through Python.
    """
    )
    return


@app.cell
def _(Viewer, build_color_grid, math, np, rr):
    def _():
        viewer = Viewer()
        viewer.display()

        recordings = [
            rr.RecordingStream("rerun_example_time_ctrl", recording_id="example_a"),
            rr.RecordingStream("rerun_example_time_ctrl", recording_id="example_b"),
        ]

        rec_colors = {"example_a": [0, 255, 0], "example_b": [255, 0, 0]}

        for rec in recordings:
            viewer.add_recording(rec)

        STEPS = 100
        twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
        for rec in recordings:
            for t in range(STEPS):
                cube = build_color_grid(10, 10, 10, twist=twists[t])
                rec.set_time("step", sequence=t)
        rec.log("cube", rr.Points3D(cube.positions, colors=rec_colors[rec.get_recording_id()], radii=0.5))
        return viewer

    another_viewer = _()
    return (another_viewer,)


@app.cell
def _(mo):
    mo.md(r"""The state of each panel in the viewer can be overridden, locking it in the specified state.""")
    return


@app.cell
def _(another_viewer):
    another_viewer.update_panels(blueprint="expanded")
    return


@app.cell
def _(mo):
    mo.md(
        r"""In multi-recording scenarios, the active recording can be set using `set_active_recording`. The timeline panel's state for the currently active recording can be controlled using `set_time_ctrl`."""
    )
    return


@app.cell
def _(another_viewer):
    another_viewer.set_active_recording(recording_id="example_a")
    another_viewer.set_time_ctrl(timeline="step", sequence=25)
    another_viewer.set_active_recording(recording_id="example_b")
    another_viewer.set_time_ctrl(timeline="step", sequence=75)
    return


@app.cell
def _(mo):
    mo.md(r"""Switch between the two recordings in the blueprint panel to see the updated timelines.""")
    return


@app.cell
def _():
    import marimo as mo

    return (mo,)


if __name__ == "__main__":
    app.run()
