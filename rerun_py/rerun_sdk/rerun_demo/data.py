"""Simple data to be used for Rerun demos."""

from collections import namedtuple
from math import cos, sin, tau

import numpy as np
from rerun.log.rects import RectFormat
from rerun_demo.turbo import turbo_colormap_data

ColorGrid = namedtuple("ColorGrid", ["positions", "colors"])


def build_color_grid(x_count=6, y_count=6, z_count=6):
    """
    Create a cube of points with colors.

    The total point cloud will have x_count * y_count * z_count points.

    Parameters
    ----------
    x_count, y_count, z_count:
        Number of points in each dimension.

    """
    positions = np.vstack(
        [
            xyz.ravel()
            for xyz in np.mgrid[
                slice(-10, 10, x_count * 1j),
                slice(-10, 10, y_count * 1j),
                slice(-10, 10, z_count * 1j),
            ]
        ]
    )

    colors = np.vstack(
        [
            xyz.ravel()
            for xyz in np.mgrid[
                slice(0, 255, x_count * 1j),
                slice(0, 255, y_count * 1j),
                slice(0, 255, z_count * 1j),
            ]
        ]
    )

    return ColorGrid(positions.T, colors.T.astype(np.uint8))


color_grid = build_color_grid()
"""Default color grid"""


RectPyramid = namedtuple("RectPyramid", ["rects", "format", "colors"])


def build_rect_pyramid(count=20, width=100, height=100):
    """
    Create a stack of N colored rectangles.

    Parameters
    ----------
    count:
        Number of rectangles to create.
    width:
        Width of the base of the pyramid.
    height:
        Height of the pyramid.

    """
    x = np.zeros(count)
    y = np.linspace(0, height, count)
    widths = np.linspace(float(width) / count, width, count)
    heights = 0.8 * np.ones(count) * height / count
    rects = np.array(list(zip(x, y, widths, heights)))
    colors = turbo_colormap_data[np.linspace(0, len(turbo_colormap_data) - 1, count, dtype=int)]

    return RectPyramid(rects, RectFormat.XCYCWH, colors)


rect_pyramid = build_rect_pyramid()
"""Default rect pyramid data"""


ColorSpiral = namedtuple("ColorSpiral", ["positions", "colors"])


def build_color_spiral(num_points=100, radius=2, angular_step=0.02, angular_offset=0, z_step=0.1):
    """
    Create a spiral of points with colors along the Z axis.

    Parameters
    ----------
    num_points:
        Total number of points.
    radius:
        The radius of the spiral.
    angular_step:
        The factor applied between each step along the trigonemetric circle.
    angular_offset:
        Offsets the starting position on the trigonemetric circle.
    z_step:
        The factor applied between between each step along the Z axis.

    """
    positions = np.array(
        [
            [
                sin(i * tau * angular_step + angular_offset) * radius,
                cos(i * tau * angular_step + angular_offset) * radius,
                i * z_step,
            ]
            for i in range(num_points)
        ]
    )
    colors = turbo_colormap_data[np.linspace(0, len(turbo_colormap_data) - 1, num_points, dtype=int)]

    return ColorSpiral(positions, colors)


color_spiral = build_color_spiral()
"""Default color spiral"""
