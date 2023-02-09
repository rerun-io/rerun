from collections import namedtuple

import numpy as np
from rerun.log.rects import RectFormat
from rerun_demo.turbo import turbo_colormap_data

ColorGrid = namedtuple("ColorGrid", ["positions", "colors"])


def build_color_grid(x_size, y_size, z_size):
    x, y, z = np.meshgrid(np.linspace(-10, 10, x_size), np.linspace(-10, 10, y_size), np.linspace(-10, 10, z_size))
    r, g, b = np.meshgrid(np.linspace(0, 255, x_size), np.linspace(0, 255, y_size), np.linspace(0, 255, z_size))
    positions = np.array(list(zip(x.reshape(-1), y.reshape(-1), z.reshape(-1))))
    colors = np.array(list(zip(r.reshape(-1), g.reshape(-1), b.reshape(-1))), dtype=np.uint8)

    return ColorGrid(positions, colors)


color_grid = build_color_grid(6, 6, 6)


RectPyramid = namedtuple("RectPyramid", ["rects", "format", "colors"])


def build_rect_pyramid(steps):
    x = np.zeros(steps)
    y = np.linspace(0, 100, steps)
    widths = np.linspace(0, 100, steps)
    heights = 0.8 * np.ones(steps) * 100 / steps
    rects = np.array(list(zip(x, y, widths, heights)))
    colors = turbo_colormap_data[np.linspace(0, len(turbo_colormap_data) - 1, steps, dtype=int)]

    return RectPyramid(rects, RectFormat.XCYCWH, colors)


rect_pyramid = build_rect_pyramid(20)
