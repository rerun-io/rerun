"""Plot Y against X (not against time) as a line strip in a 2D view."""

import numpy as np

import rerun as rr

rr.init("rerun_example_spatial2d_xy_plot", spawn=True)

x = np.linspace(-3.0, 3.0, 200)
y = np.sin(x) * x
rr.log("xy_plot", rr.LineStrips2D([np.column_stack([x, -y])]))
