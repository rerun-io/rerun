"""Log some very simple points."""
from __future__ import annotations

import rerun as rr

rr.init("points", spawn=True)

rr.log_points("simple", positions=[[0, 0], [1, 1]])

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 4, 4], rect_format=rr.RectFormat.XCYCWH)
