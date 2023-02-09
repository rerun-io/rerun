"""Simple data to be used for rerun demos."""

from collections import namedtuple

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
    x, y, z = np.meshgrid(np.linspace(-10, 10, x_count), np.linspace(-10, 10, y_count), np.linspace(-10, 10, z_count))
    r, g, b = np.meshgrid(np.linspace(0, 255, x_count), np.linspace(0, 255, y_count), np.linspace(0, 255, z_count))
    positions = np.array(list(zip(x.reshape(-1), y.reshape(-1), z.reshape(-1))))
    colors = np.array(list(zip(r.reshape(-1), g.reshape(-1), b.reshape(-1))), dtype=np.uint8)

    return ColorGrid(positions, colors)


color_grid = build_color_grid()


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
