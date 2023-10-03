from __future__ import annotations

from typing import Any

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import TimeSeriesScalar
from rerun.log_deprecated import Color, _normalize_colors
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_scalar",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.TimeSeriesScalar(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_scalar(
    entity_path: str,
    scalar: float,
    *,
    label: str | None = None,
    color: Color | None = None,
    radius: float | None = None,
    scattered: bool | None = None,
    ext: dict[str, Any] | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a double-precision scalar that will be visualized as a timeseries plot.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.TimeSeriesScalar][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
        TODO(#1289). If all points
        within a single entity path (i.e. a line) share the same label, then
        this label will be used as the label for the line itself. Otherwise, the
        line will be named after the entity path. The plot itself is named after
        the space it's in.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.

        If left unspecified, a pseudo-random color will be used instead. That
        same color will apply to all points residing in the same entity path
        that don't have a color specified.

        Points within a single line do not have to share the same color, the line
        will have differently colored segments as appropriate.
        If all points within a single entity path (i.e. a line) share the same
        color, then this color will be used as the line color in the plot legend.
        Otherwise, the line will appear gray in the legend.
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
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    if color is not None:
        color = _normalize_colors(color)

    return log(
        entity_path,
        TimeSeriesScalar(scalar=scalar, label=label, color=color, radius=radius, scattered=scattered),
        AnyValues(**(ext or {})),
        recording=recording,
    )
