from __future__ import annotations

import rerun as rr
import rerun.experimental as rr_exp


def test_log_point2d_basic() -> None:
    """Basic test: logging a point shouldn't raise an exception..."""
    points = rr_exp.Points2D([(0, 0), (2, 2), (2, 2.5), (2.5, 2), (3, 4)], radii=0.5)
    rr.init("test_log")
    rr_exp.log_any("points", points)
