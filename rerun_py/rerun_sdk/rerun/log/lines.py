from __future__ import annotations

from typing import Any, Iterable

import numpy as np
import numpy.typing as npt
from deprecated import deprecated

from rerun import bindings
from rerun.components.color import ColorRGBAArray
from rerun.components.draw_order import DrawOrderArray
from rerun.components.instance import InstanceArray
from rerun.components.linestrip import LineStrip2DArray, LineStrip3DArray
from rerun.components.radius import RadiusArray
from rerun.log import Color, Colors, _normalize_colors, _normalize_radii
from rerun.log.error_utils import _send_warning
from rerun.log.extension_components import _add_extension_components
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_path",
    "log_line_strip",
    "log_line_strips_2d",
    "log_line_strips_3d",
    "log_line_segments",
]


@deprecated(version="0.2.0", reason="Use log_line_strip instead")
def log_path(
    entity_path: str,
    positions: npt.ArrayLike | None,
    *,
    stroke_width: float | None = None,
    color: Color | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    log_line_strip(
        entity_path, positions, stroke_width=stroke_width, color=color, ext=ext, timeless=timeless, recording=recording
    )


@log_decorator
def log_line_strip(
    entity_path: str,
    positions: npt.ArrayLike | None,
    *,
    stroke_width: float | None = None,
    color: Color | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Log a line strip through 2D or 3D space.

    A line strip is a list of points connected by line segments. It can be used to draw approximations of smooth curves.

    The points will be connected in order, like so:
    ```
           2------3     5
          /        \   /
    0----1          \ /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the path in the space hierarchy
    positions:
        An Nx2 or Nx3 array of points along the path.
    stroke_width:
        Optional width of the line.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for lines is 20.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the path will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    if positions is not None:
        positions = np.require(positions, dtype="float32")

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if positions is not None:
        if positions.shape[1] == 2:
            instanced["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays([positions])
        elif positions.shape[1] == 3:
            instanced["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays([positions])
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    if color is not None:
        colors = _normalize_colors(color)
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

    if draw_order is not None:
        instanced["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@log_decorator
def log_line_strips_2d(
    entity_path: str,
    line_strips: Iterable[npt.ArrayLike] | None,
    *,
    identifiers: npt.ArrayLike | None = None,
    stroke_widths: npt.ArrayLike | None = None,
    colors: Color | Colors | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Log a batch of line strips through 2D space.

    Each line strip is a list of points connected by line segments. It can be used to draw
    approximations of smooth curves.

    The points will be connected in order, like so:
    ```
           2------3     5
          /        \   /
    0----1          \ /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the path in the space hierarchy
    line_strips:
        An iterable of Nx2 arrays of points along the path.
        To log an empty line_strip use `np.zeros((0,0,3))` or `np.zeros((0,0,2))`
    identifiers:
        Unique numeric id that shows up when you hover or select the line.
    stroke_widths:
        Optional widths of the line.
    colors:
        Optional colors of the lines.
        RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for lines is 20.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the path will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    colors = _normalize_colors(colors)
    stroke_widths = _normalize_radii(stroke_widths)
    radii = stroke_widths / 2.0

    identifiers_np = np.array((), dtype="uint64")
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]

    if line_strips is not None:
        line_strip_arrs = [np.require(line, dtype="float32") for line in line_strips]
        dims = [line.shape[1] for line in line_strip_arrs]

        if any(d != 2 for d in dims):
            raise ValueError("All line strips must be Nx2")

        comps[0]["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays(line_strip_arrs)

    if len(identifiers_np):
        comps[0]["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    if len(colors):
        is_splat = len(colors.shape) == 1
        if is_splat:
            colors = colors.reshape(1, len(colors))
        comps[is_splat]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if len(radii):
        is_splat = len(radii) == 1
        comps[is_splat]["rerun.radius"] = RadiusArray.from_numpy(radii)

    if draw_order is not None:
        comps[1]["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(comps[0], comps[1], ext, identifiers_np)

    if comps[1]:
        comps[1]["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless, recording=recording)


@log_decorator
def log_line_strips_3d(
    entity_path: str,
    line_strips: Iterable[npt.ArrayLike] | None,
    *,
    identifiers: npt.ArrayLike | None = None,
    stroke_widths: npt.ArrayLike | None = None,
    colors: Color | Colors | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Log a batch of line strips through 3D space.

    Each line strip is a list of points connected by line segments. It can be used to draw approximations
    of smooth curves.

    The points will be connected in order, like so:
    ```
           2------3     5
          /        \   /
    0----1          \ /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the path in the space hierarchy
    line_strips:
        An iterable of Nx3 arrays of points along the path.
        To log an empty line_strip use `np.zeros((0,0,3))` or `np.zeros((0,0,2))`
    identifiers:
        Unique numeric id that shows up when you hover or select the line.
    stroke_widths:
        Optional widths of the line.
    colors:
        Optional colors of the lines.
        RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for lines is 20.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the path will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    colors = _normalize_colors(colors)
    stroke_widths = _normalize_radii(stroke_widths)
    radii = stroke_widths / 2.0

    identifiers_np = np.array((), dtype="uint64")
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]

    if line_strips is not None:
        line_strip_arrs = [np.require(line, dtype="float32") for line in line_strips]
        dims = [line.shape[1] for line in line_strip_arrs]

        if any(d != 3 for d in dims):
            raise ValueError("All line strips must be Nx3")

        comps[0]["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays(line_strip_arrs)

    if len(identifiers_np):
        comps[0]["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    if len(colors):
        is_splat = len(colors.shape) == 1
        if is_splat:
            colors = colors.reshape(1, len(colors))
        comps[is_splat]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if len(radii):
        is_splat = len(radii) == 1
        comps[is_splat]["rerun.radius"] = RadiusArray.from_numpy(radii)

    if draw_order is not None:
        comps[1]["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(comps[0], comps[1], ext, identifiers_np)

    if comps[1]:
        comps[1]["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless, recording=recording)


@log_decorator
def log_line_segments(
    entity_path: str,
    positions: npt.ArrayLike,
    *,
    stroke_width: float | None = None,
    color: Color | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Log many 2D or 3D line segments.

    The points will be connected in even-odd pairs, like so:

    ```
           2------3     5
                       /
    0----1            /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the line segments in the space hierarchy
    positions:
        An Nx2 or Nx3 array of points. Even-odd pairs will be connected as segments.
    stroke_width:
        Optional width of the line.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for lines is 20.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the line segments will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    if positions is None:
        positions = np.require([], dtype="float32")
    positions = np.require(positions, dtype="float32")

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if positions is not None:
        # If not a multiple of 2, drop the last row
        if len(positions) % 2:
            positions = positions[:-1]
        if positions.ndim > 1 and positions.shape[1] == 2:
            # Reshape even-odd pairs into a collection of line-strips of length2
            # [[a00, a01], [a10, a11], [b00, b01], [b10, b11]]
            # -> [[[a00, a01], [a10, a11]], [[b00, b01], [b10, b11]]]
            positions = positions.reshape([len(positions) // 2, 2, 2])
            instanced["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays(positions)
        elif positions.ndim > 1 and positions.shape[1] == 3:
            # Same as above but for 3d points
            positions = positions.reshape([len(positions) // 2, 2, 3])
            instanced["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays(positions)
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    # The current API splats both color and stroke-width, though the data-model doesn't
    # require that we do so.
    if color is not None:
        colors = _normalize_colors(color)
        splats["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        splats["rerun.radius"] = RadiusArray.from_numpy(radii)

    if draw_order is not None:
        instanced["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)
