from typing import Any, Dict, Optional, Sequence

import numpy as np
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.label import LabelArray
from rerun.components.radius import RadiusArray
from rerun.components.scalar import ScalarArray, ScalarPlotPropsArray
from rerun.log import _normalize_colors
from rerun.log.extension_components import _add_extension_components

from rerun import bindings

__all__ = [
    "log_scalar",
]


def log_scalar(
    entity_path: str,
    scalar: float,
    label: Optional[str] = None,
    color: Optional[Sequence[int]] = None,
    radius: Optional[float] = None,
    scattered: Optional[bool] = None,
    ext: Optional[Dict[str, Any]] = None,
) -> None:
    """
    Log a double-precision scalar that will be visualized as a timeseries plot.

    The current simulation time will be used for the time/X-axis, hence scalars
    cannot be timeless!

    See [here](https://github.com/rerun-io/rerun/blob/main/examples/python/plots/main.py) for a larger example.

    Understanding the plot and attributes hierarchy
    -----------------------------------------------

    Timeseries come in three parts: points, lines and finally the plots
    themselves. As a user of the Rerun SDK, your one and only entrypoint into
    that hierarchy is through the lowest-level layer: the points.

    When logging scalars and their attributes (label, color, radius, scattered)
    through this function, Rerun will turn them into points with similar
    attributes. From these points, lines with appropriate attributes can then be
    inferred; and from these inferred lines, plots with appropriate attributes
    will be inferred in turn!

    In terms of actual hierarchy:

    - Each space represents a single plot.
    - Each entity path within a space that contains scalar data is a line within that plot.
    - Each logged scalar is a point.

    E.g. the following:
    ```
    t=1.0
    rerun.log_scalar("trig/sin", math.sin(t), label="sin(t)", color=[255, 0, 0])
    rerun.log_scalar("trig/cos", math.cos(t), label="cos(t)", color=[0, 0, 255])
    ```
    will yield a single plot (space = `trig`), comprised of two lines
    (entity paths `trig/sin` and `trig/cos`).


    Parameters
    ----------
    entity_path:
        The path to the scalar in the space hierarchy.
    scalar:
        The scalar value to log.
    label:
        An optional label for the point.

        This won't show up on points at the moment, as our plots don't yet
        support displaying labels for individual points
        TODO(https://github.com/rerun-io/rerun/issues/1289). If all points
        within a single entity path (i.e. a line) share the same label, then
        this label will be used as the label for the line itself. Otherwise, the
        line will be named after the entity path. The plot itself is named after
        the space it's in.
    color:
        An optional color in the form of a RGB or RGBA triplet in 0-255 sRGB.

        If left unspecified, a pseudo-random color will be used instead. That
        same color will apply to all points residing in the same entity path
        that don't have a color specified.

        Points within a single line do not have to share the same color, the line
        will have differently colored segments as appropriate.
        If all points within a single entity path (i.e. a line) share the same
        color, then this color will be used as the line color in the plot legend.
        Otherwise, the line will appear grey in the legend.
    radius:
        An optional radius for the point.

        Points within a single line do not have to share the same radius, the line
        will have differently sized segments as appropriate.

        If all points within a single entity path (i.e. a line) share the same
        radius, then this radius will be used as the line width too. Otherwise, the
        line will use the default width of `1.0`.
    scattered:
        Specifies whether the point should form a continuous line with its
        neighbors, or whether it should stand on its own, akin to a scatter plot.
        Points within a single line do not have to all share the same scatteredness:
        the line will switch between a scattered and a continuous representation as
        required.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]

    """

    if not bindings.is_enabled():
        return

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    instanced["rerun.scalar"] = ScalarArray.from_numpy(np.array([scalar]))

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if color:
        colors = _normalize_colors(np.array([color]))
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if radius:
        instanced["rerun.radius"] = RadiusArray.from_numpy(np.array([radius]))

    if scattered:
        props = [{"scattered": scattered}]
        instanced["rerun.scalar_plot_props"] = ScalarPlotPropsArray.from_props(props)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=False)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=False)
