"""Simple data to be used for Rerun demos."""
from __future__ import annotations

from collections import namedtuple
from math import cos, sin, tau

import numpy as np

from rerun import Box2DFormat

from .turbo import turbo_colormap_data

__all__ = [
    "ColorGrid",
    "build_color_grid",
    "color_grid",
    "RectPyramid",
    "build_rect_pyramid",
    "rect_pyramid",
    "ColorSpiral",
    "build_color_spiral",
    "color_spiral",
]

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
        slice(-10, 10, x_count * 1j),
        slice(-10, 10, y_count * 1j),
        slice(-10, 10, z_count * 1j),
    ]

    angle = np.linspace(-float(twist) / 2, float(twist) / 2, z_count)
    for z in range(z_count):
        xv, yv, zv = grid[:, :, :, z]
        rot_xv = xv * cos(angle[z]) - yv * sin(angle[z])
        rot_yv = xv * sin(angle[z]) + yv * cos(angle[z])
        grid[:, :, :, z] = [rot_xv, rot_yv, zv]

    positions = np.vstack([xyz.ravel() for xyz in grid])

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


def build_rect_pyramid(count: int = 20, width: int = 100, height: int = 100) -> RectPyramid:
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
    rects = np.stack((x, y, widths, heights), axis=1)
    colors = turbo_colormap_data[np.linspace(0, len(turbo_colormap_data) - 1, count, dtype=int)]

    return RectPyramid(rects, Box2DFormat.XCYCWH, colors)


rect_pyramid = build_rect_pyramid()
"""Default rect pyramid data"""


ColorSpiral = namedtuple("ColorSpiral", ["positions", "colors"])


def build_color_spiral(
    num_points: int = 100,
    radius: float = 2,
    angular_step: float = 0.02,
    angular_offset: float = 0,
    z_step: float = 0.1,
) -> ColorSpiral:
    """
    Create a spiral of points with colors along the Z axis.

    Parameters
    ----------
    num_points:
        Total number of points.
    radius:
        The radius of the spiral.
    angular_step:
        The factor applied between each step along the trigonometric circle.
    angular_offset:
        Offsets the starting position on the trigonometric circle.
    z_step:
        The factor applied between each step along the Z axis.

    """
    positions = np.array(
        [
            [
                cos(i * tau * angular_step + angular_offset) * radius,
                sin(i * tau * angular_step + angular_offset) * radius,
                i * z_step,
            ]
            for i in range(num_points)
        ]
    )
    colors = turbo_colormap_data[np.linspace(0, len(turbo_colormap_data) - 1, num_points, dtype=int)]

    return ColorSpiral(positions, colors)


color_spiral = build_color_spiral()
"""Default color spiral"""
