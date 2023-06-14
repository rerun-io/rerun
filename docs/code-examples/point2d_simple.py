"""Log some very simple points."""
from __future__ import annotations

import rerun as rr

rr.init("points", spawn=True)

rr.log_points("simple", positions=[[0, 0], [1, 1]])
