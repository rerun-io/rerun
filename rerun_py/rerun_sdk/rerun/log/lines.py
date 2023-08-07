from __future__ import annotations

from typing import Any, Iterable

import numpy as np
import numpy.typing as npt

from rerun.log import Color, Colors, _normalize_radii
from rerun.log.error_utils import _send_warning
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_line_strip",
    "log_line_strips_2d",
    "log_line_strips_3d",
    "log_line_segments",
]


@log_decorator
def log_line_strip(
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
    from rerun.experimental import LineStrips2D, LineStrips3D, log

    if positions is None:
        raise ValueError("`positions` argument must be set")

    recording = RecordingStream.to_native(recording)

    stroke_widths = _normalize_radii(stroke_width)
    radii = stroke_widths / 2.0

    positions = np.require(positions, dtype="float32")
    if positions.shape[1] == 2:
        strips2d = LineStrips2D(
            [positions],
            radii=radii,
            colors=color,
            draw_order=draw_order,
        )
        return log(entity_path, strips2d, ext=ext, timeless=timeless, recording=recording)
    elif positions.shape[1] == 3:
        strips3d = LineStrips3D(
            [positions],
            radii=radii,
            colors=color,
        )
        return log(entity_path, strips3d, ext=ext, timeless=timeless, recording=recording)
    else:
        raise TypeError("Positions should be either Nx2 or Nx3")


@log_decorator
def log_line_strips_2d(
    entity_path: str,
    line_strips: Iterable[npt.ArrayLike],
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
    from rerun.experimental import LineStrips2D, log

    if line_strips is None:
        raise ValueError("`line_strips` argument must be set")

    recording = RecordingStream.to_native(recording)

    identifiers_np = np.array((), dtype="uint64")
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    stroke_widths = _normalize_radii(stroke_widths)
    radii = stroke_widths / 2.0

    arch = LineStrips2D(
        line_strips,
        radii=radii,
        colors=colors,
        draw_order=draw_order,
        instance_keys=identifiers_np,
    )
    return log(entity_path, arch, ext=ext, timeless=timeless, recording=recording)


@log_decorator
def log_line_strips_3d(
    entity_path: str,
    line_strips: Iterable[npt.ArrayLike],
    *,
    identifiers: npt.ArrayLike | None = None,
    stroke_widths: npt.ArrayLike | None = None,
    colors: Color | Colors | None = None,
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
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the path will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    from rerun.experimental import LineStrips3D, log

    if line_strips is None:
        raise ValueError("`line_strips` argument must be set")

    recording = RecordingStream.to_native(recording)

    identifiers_np = np.array((), dtype="uint64")
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    stroke_widths = _normalize_radii(stroke_widths)
    radii = stroke_widths / 2.0

    arch = LineStrips3D(
        line_strips,
        radii=radii,
        colors=colors,
        instance_keys=identifiers_np,
    )
    return log(entity_path, arch, ext=ext, timeless=timeless, recording=recording)


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
    from rerun.experimental import LineStrips2D, LineStrips3D, log

    if positions is None:
        raise ValueError("`positions` argument must be set")

    recording = RecordingStream.to_native(recording)

    positions = np.require(positions, dtype="float32")
    # If not a multiple of 2, drop the last row
    if len(positions) % 2:
        positions = positions[:-1]
    if positions.ndim > 1 and positions.shape[1] == 2:
        # Reshape even-odd pairs into a collection of line-strips of length2
        # [[a00, a01], [a10, a11], [b00, b01], [b10, b11]]
        # -> [[[a00, a01], [a10, a11]], [[b00, b01], [b10, b11]]]
        positions = positions.reshape([len(positions) // 2, 2, 2])
        strips2d = LineStrips2D(
            positions,
            radii=stroke_width * 0.5 if stroke_width is not None else None,
            colors=color,
            draw_order=draw_order,
        )
        return log(entity_path, strips2d, ext=ext, timeless=timeless, recording=recording)
    elif positions.ndim > 1 and positions.shape[1] == 3:
        # Same as above but for 3d points
        positions = positions.reshape([len(positions) // 2, 2, 3])
        strips3d = LineStrips3D(
            positions,
            radii=stroke_width * 0.5 if stroke_width is not None else None,
            colors=color,
        )
        return log(entity_path, strips3d, ext=ext, timeless=timeless, recording=recording)
    else:
        raise TypeError("Positions should be either Nx2 or Nx3")
