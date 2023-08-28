from __future__ import annotations

import rerun as rr
import rerun.experimental as rr2


def test_log_point2d_basic() -> None:
    """Basic test: logging a point shouldn't raise an exception..."""
    points = rr2.Points2D([(0, 0), (2, 2), (2, 2.5), (2.5, 2), (3, 4)], radii=0.5)
    rr.init("rerun_example_test_log")
    rr2.log("points", points)
